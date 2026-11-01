//! M11-PKG packaging helper.
//!
//! This is a standalone binary. It deliberately does not touch wolf-cli's main
//! entry point:
//!
//!   validate  check that a WOLF data root has the required directories and .dat files
//!   assemble  stage wolf-cli.exe plus its data root and write manifest.json
//!   check     verify an existing package against manifest.json
//!   bench     time a real Game::frame loop and print p50/p95/max microseconds
//!
//! The manifest is ordinary JSON with a stable flat file list, each entry carrying
//! a relative path, byte size and SHA-256 digest. The package is self-contained:
//! wolf-cli.exe at the root, data root under data/, manifest.json beside them.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use wolf_game::frame_budget::measure_game;
use wolf_game::Game;

const REQUIRED_DIRS: &[&str] = &["BasicData", "MapData", "SystemFile"];
const REQUIRED_FILES: &[&str] = &[
    "BasicData/Game.dat",
    "BasicData/CommonEvent.dat",
    "BasicData/CDataBase.dat",
    "BasicData/DataBase.dat",
    "BasicData/SysDatabase.dat",
    "BasicData/SysDataBaseBasic.dat",
    "BasicData/TileSetData.dat",
    "BasicData/MapTree.dat",
];
const MANIFEST_NAME: &str = "manifest.json";
const MANIFEST_FORMAT: u32 = 1;
const PACKAGE_DATA_DIR: &str = "data";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("wolf-package: {message}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> &'static str {
    "usage:
  wolf-package validate [--root PATH]
  wolf-package assemble --root PATH --exe PATH --out DIR [--runtime FILE ...]
  wolf-package check --package DIR
  wolf-package --check DIR
  wolf-package bench [--root PATH] [--save FILE] [--event ID] [--frames N] [--warmup N]

PATH defaults to WOLF_DATA_ROOT, then work/all. N defaults to 600 frames
after 60 warm-up frames. assemble copies the whole data root under data/ and
copies the executable to wolf-cli.exe; adjacent .dll files are not guessed,
pass each one with --runtime. No GUI and no network access are used."
}

fn run() -> Result<(), String> {
    let mut raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() {
        println!("{}", usage());
        return Ok(());
    }
    let mut command = raw.remove(0);
    let mut package = None;
    if command == "--check" {
        command = "check".to_string();
        if !raw.is_empty() && !raw[0].starts_with("--") {
            package = Some(PathBuf::from(raw.remove(0)));
        }
    }
    let mut root = std::env::var_os("WOLF_DATA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("work/all"));
    let mut exe = None;
    let mut out = PathBuf::from("work/out/package");
    let mut runtimes = Vec::new();
    let mut save = None;
    let mut event = 188u32;
    let mut frames = 600usize;
    let mut warmup = 60usize;

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--root" => {
                root = PathBuf::from(value(&raw, &mut i, "--root")?);
            }
            "--exe" => {
                exe = Some(PathBuf::from(value(&raw, &mut i, "--exe")?));
            }
            "--out" => {
                out = PathBuf::from(value(&raw, &mut i, "--out")?);
            }
            "--runtime" => {
                runtimes.push(PathBuf::from(value(&raw, &mut i, "--runtime")?));
            }
            "--package" => {
                package = Some(PathBuf::from(value(&raw, &mut i, "--package")?));
            }
            "--save" => {
                save = Some(PathBuf::from(value(&raw, &mut i, "--save")?));
            }
            "--event" => {
                event = value(&raw, &mut i, "--event")?
                    .parse()
                    .map_err(|_| "--event needs an integer".to_string())?;
            }
            "--frames" => {
                frames = value(&raw, &mut i, "--frames")?
                    .parse()
                    .map_err(|_| "--frames needs a positive integer".to_string())?;
                if frames == 0 {
                    return Err("--frames must be greater than zero".to_string());
                }
            }
            "--warmup" => {
                warmup = value(&raw, &mut i, "--warmup")?
                    .parse()
                    .map_err(|_| "--warmup needs a non-negative integer".to_string())?;
            }
            "-h" | "--help" | "help" => {
                println!("{}", usage());
                return Ok(());
            }
            other => return Err(format!("unknown option {other}")),
        }
        i += 1;
    }

    match command.as_str() {
        "validate" => run_validate(&root),
        "assemble" => run_assemble(&root, exe.as_deref(), &out, &runtimes),
        "check" => {
            let package = package.ok_or_else(|| "check needs --package DIR".to_string())?;
            run_check(&package)
        }
        "bench" => run_bench(&root, save.as_deref(), event, frames, warmup),
        other => Err(format!("unknown command {other}; try --help")),
    }
}

fn value(raw: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    if *index + 1 >= raw.len() {
        return Err(format!("{flag} needs a value"));
    }
    *index += 1;
    Ok(raw[*index].clone())
}

#[derive(Debug, Default)]
struct Validation {
    root: PathBuf,
    missing: Vec<String>,
    empty: Vec<String>,
    extra: Vec<String>,
    dirs_present: usize,
    files_present: usize,
    file_count: u64,
    total_bytes: u64,
}

impl Validation {
    fn ok(&self) -> bool {
        self.missing.is_empty() && self.empty.is_empty()
    }
}

fn run_validate(root: &Path) -> Result<(), String> {
    let report =
        validate_root(root).map_err(|error| format!("cannot inspect {root:?}: {error}"))?;
    print_validation(&report);
    if report.ok() {
        println!(
            "VALID: data root is complete ({} files, {} bytes)",
            report.file_count, report.total_bytes
        );
        Ok(())
    } else {
        Err("data root validation failed".to_string())
    }
}

fn validate_root(root: &Path) -> io::Result<Validation> {
    let mut report = Validation {
        root: root.to_path_buf(),
        ..Validation::default()
    };
    if !root.is_dir() {
        report
            .missing
            .push(format!("{} (root directory)", root.display()));
        return Ok(report);
    }
    for dir in REQUIRED_DIRS {
        let path = root.join(dir);
        if !path.is_dir() {
            report.missing.push((*dir).to_string());
        } else {
            report.dirs_present += 1;
            if count_files(&path)? == 0 {
                report.empty.push(format!("{dir}/ (directory is empty)"));
            }
        }
    }
    for file in REQUIRED_FILES {
        let path = root.join(file);
        if !path.is_file() {
            report.missing.push((*file).to_string());
        } else {
            let length = fs::metadata(&path)?.len();
            report.files_present += 1;
            if length == 0 {
                report.empty.push(format!("{file} (zero bytes)"));
            }
        }
    }
    let (file_count, total_bytes) = tree_stats(root)?;
    report.file_count = file_count;
    report.total_bytes = total_bytes;
    report.extra = extra_top_level(root)?;
    Ok(report)
}

fn count_files(root: &Path) -> io::Result<u64> {
    let mut count = 0;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            count += count_files(&entry.path())?;
        } else if kind.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

fn tree_stats(root: &Path) -> io::Result<(u64, u64)> {
    let mut count = 0;
    let mut bytes = 0;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            let (child_count, child_bytes) = tree_stats(&entry.path())?;
            count += child_count;
            bytes += child_bytes;
        } else if kind.is_file() {
            count += 1;
            bytes += entry.metadata()?.len();
        }
    }
    Ok((count, bytes))
}

fn extra_top_level(root: &Path) -> io::Result<Vec<String>> {
    let mut extra = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_known_top_level(&name) {
            extra.push(name);
        }
    }
    extra.sort();
    Ok(extra)
}

fn is_known_top_level(name: &str) -> bool {
    const KNOWN: &[&str] = &[
        "BasicData",
        "MapData",
        "SystemFile",
        "Save",
        "BGM",
        "SE",
        "Picture",
        "SystemGraphic",
        "MapChip",
        "CharaChip",
        "BattleEffect",
        "EnemyGraphic",
        "FIcon",
        "Fog_BackGround",
        "Kagura",
        "DLC_data",
        "1.4.0",
        "BalloonBase_CafeLatte",
        "cg",
        "cutin",
        "hscene",
        "hword",
        "scene",
        "word",
    ];
    KNOWN.contains(&name) || name.ends_with(".wolf")
}

fn print_validation(report: &Validation) {
    println!("wolf-package validate: root = {}", report.root.display());
    println!(
        "  required dirs : {}/{} present",
        report.dirs_present,
        REQUIRED_DIRS.len()
    );
    println!(
        "  required files: {}/{} present",
        report.files_present,
        REQUIRED_FILES.len()
    );
    println!(
        "  data          : {} files, {} bytes",
        report.file_count, report.total_bytes
    );
    if !report.missing.is_empty() {
        println!("MISSING ({}):", report.missing.len());
        for item in &report.missing {
            println!("  {item}");
        }
    }
    if !report.empty.is_empty() {
        println!("EMPTY ({}):", report.empty.len());
        for item in &report.empty {
            println!("  {item}");
        }
    }
    if report.extra.is_empty() {
        println!("  extra top-level: none");
    } else {
        println!("EXTRA top-level ({}):", report.extra.len());
        for item in &report.extra {
            println!("  {item}");
        }
    }
}

fn run_assemble(
    root: &Path,
    exe: Option<&Path>,
    out: &Path,
    runtimes: &[PathBuf],
) -> Result<(), String> {
    let report =
        validate_root(root).map_err(|error| format!("cannot inspect {root:?}: {error}"))?;
    print_validation(&report);
    if !report.ok() {
        return Err("cannot assemble from an invalid data root".to_string());
    }
    let exe = match exe {
        Some(path) => path.to_path_buf(),
        None => default_exe()?,
    };
    if !exe.is_file() {
        return Err(format!("executable not found: {}", exe.display()));
    }
    if out.starts_with(root) {
        return Err("output directory must not be inside the data root".to_string());
    }
    if out.exists()
        && fs::read_dir(out)
            .map_err(|error| error.to_string())?
            .next()
            .is_some()
    {
        return Err(format!(
            "output directory is not empty: {} (choose a fresh directory)",
            out.display()
        ));
    }
    fs::create_dir_all(out).map_err(|error| format!("create {}: {error}", out.display()))?;
    let exe_dest = out.join("wolf-cli.exe");
    copy_file(&exe, &exe_dest)?;
    let mut destinations = BTreeSet::new();
    destinations.insert(exe_dest);
    for runtime in runtimes {
        let name = runtime
            .file_name()
            .ok_or_else(|| format!("runtime has no file name: {}", runtime.display()))?;
        let dest = out.join(name);
        if !destinations.insert(dest.clone()) {
            return Err(format!("duplicate runtime destination: {}", dest.display()));
        }
        copy_file(runtime, &dest)?;
    }
    copy_tree(root, &out.join(PACKAGE_DATA_DIR))?;
    let entries = collect_manifest_entries(out)?;
    let manifest = Manifest {
        format: MANIFEST_FORMAT,
        tool: "wolf-package".to_string(),
        data_root: PACKAGE_DATA_DIR.to_string(),
        entries,
    };
    let manifest_path = out.join(MANIFEST_NAME);
    fs::write(&manifest_path, manifest.to_json())
        .map_err(|error| format!("write {}: {error}", manifest_path.display()))?;
    println!(
        "ASSEMBLED: {} files, {} bytes, output = {}",
        manifest.entries.len(),
        manifest.total_bytes(),
        out.display()
    );
    println!("  manifest: {}", manifest_path.display());
    Ok(())
}

fn default_exe() -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|error| format!("current_exe: {error}"))?;
    let sibling = current.with_file_name(if cfg!(windows) {
        "wolf-cli.exe"
    } else {
        "wolf-cli"
    });
    if sibling.is_file() {
        Ok(sibling)
    } else {
        Err("cannot find wolf-cli beside wolf-package; pass --exe PATH".to_string())
    }
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("copy {} -> {}: {error}", from.display(), to.display()))
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("create {}: {error}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|error| format!("read {}: {error}", from.display()))? {
        let entry = entry.map_err(|error| format!("read {}: {error}", from.display()))?;
        let source = entry.path();
        let dest = to.join(entry.file_name());
        let kind = entry
            .file_type()
            .map_err(|error| format!("stat {}: {error}", source.display()))?;
        if kind.is_dir() {
            copy_tree(&source, &dest)?;
        } else if kind.is_file() {
            copy_file(&source, &dest)?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct ManifestEntry {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug)]
struct Manifest {
    format: u32,
    tool: String,
    data_root: String,
    entries: Vec<ManifestEntry>,
}

impl Manifest {
    fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }

    fn to_json(&self) -> String {
        let files = self
            .entries
            .iter()
            .map(|entry| {
                format!(
                    r#"    {{"path": "{}", "bytes": {}, "sha256": "{}"}}"#,
                    json_escape(&entry.path),
                    entry.bytes,
                    entry.sha256
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");
        format!(
            r#"{{
  "format": {},
  "tool": "{}",
  "data_root": "{}",
  "file_count": {},
  "total_bytes": {},
  "files": [
{}
  ]
}}"#,
            self.format,
            json_escape(&self.tool),
            json_escape(&self.data_root),
            self.entries.len(),
            self.total_bytes(),
            files
        )
    }
    fn from_json(text: &str) -> Result<Self, String> {
        let value = parse_json(text)?;
        let object = value
            .as_object()
            .ok_or_else(|| "manifest root is not a JSON object".to_string())?;
        let format = get_number(object, "format")? as u32;
        let tool = get_string(object, "tool")?.to_string();
        let data_root = get_string(object, "data_root")?.to_string();
        let files = object
            .iter()
            .find(|(key, _)| key == "files")
            .and_then(|(_, value)| value.as_array())
            .ok_or_else(|| "manifest has no files array".to_string())?;
        let mut entries = Vec::with_capacity(files.len());
        for item in files {
            let object = item
                .as_object()
                .ok_or_else(|| "manifest file entry is not an object".to_string())?;
            entries.push(ManifestEntry {
                path: get_string(object, "path")?.to_string(),
                bytes: get_number(object, "bytes")? as u64,
                sha256: get_string(object, "sha256")?.to_string(),
            });
        }
        Ok(Self {
            format,
            tool,
            data_root,
            entries,
        })
    }
}

fn collect_manifest_entries(package: &Path) -> Result<Vec<ManifestEntry>, String> {
    let mut files = Vec::new();
    walk_files(package, package, &mut |relative, absolute| {
        let (bytes, sha256) = hash_file(absolute)?;
        files.push(ManifestEntry {
            path: relative,
            bytes,
            sha256,
        });
        Ok(())
    })?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn walk_files(
    root: &Path,
    current: &Path,
    visit: &mut dyn FnMut(String, &Path) -> Result<(), String>,
) -> Result<(), String> {
    for entry in
        fs::read_dir(current).map_err(|error| format!("read {}: {error}", current.display()))?
    {
        let entry = entry.map_err(|error| format!("read {}: {error}", current.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("stat {}: {error}", path.display()))?;
        if kind.is_dir() {
            walk_files(root, &path, visit)?;
        } else if kind.is_file() {
            let relative = relative_path(root, &path)?;
            if relative != MANIFEST_NAME {
                visit(relative, &path)?;
            }
        }
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| format!("{} is not under {}", path.display(), root.display()))?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().replace('\\', "/")),
            _ => {
                return Err(format!(
                    "unsafe package path component in {}",
                    path.display()
                ))
            }
        }
    }
    Ok(parts.join("/"))
}

fn run_check(package: &Path) -> Result<(), String> {
    let manifest_path = package.join(MANIFEST_NAME);
    let text = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let manifest = Manifest::from_json(&text).map_err(|error| format!("manifest: {error}"))?;
    if manifest.format != MANIFEST_FORMAT {
        return Err(format!(
            "manifest format {} is not supported (want {})",
            manifest.format, MANIFEST_FORMAT
        ));
    }
    let mut missing = Vec::new();
    let mut mismatched = Vec::new();
    let mut expected = BTreeSet::new();
    for entry in &manifest.entries {
        expected.insert(entry.path.clone());
        let path = package.join(&entry.path);
        if !path.is_file() {
            missing.push(entry.path.clone());
            continue;
        }
        let bytes = fs::metadata(&path)
            .map_err(|error| format!("stat {}: {error}", path.display()))?
            .len();
        if bytes != entry.bytes {
            mismatched.push(format!(
                "{}: size {} != manifest {}",
                entry.path, bytes, entry.bytes
            ));
            continue;
        }
        let (actual_bytes, actual_hash) = hash_file(&path)?;
        if actual_bytes != entry.bytes || actual_hash != entry.sha256 {
            mismatched.push(format!("{}: sha256 mismatch", entry.path));
        }
    }
    let mut actual = BTreeSet::new();
    walk_files(package, package, &mut |relative, _| {
        actual.insert(relative);
        Ok(())
    })?;
    let extra: Vec<String> = actual.difference(&expected).cloned().collect();
    println!("wolf-package check: package = {}", package.display());
    println!("  manifest entries: {}", manifest.entries.len());
    if !missing.is_empty() {
        println!("MISSING ({}):", missing.len());
        for item in &missing {
            println!("  {item}");
        }
    }
    if !mismatched.is_empty() {
        println!("MISMATCHED ({}):", mismatched.len());
        for item in &mismatched {
            println!("  {item}");
        }
    }
    if extra.is_empty() {
        println!("  extra package files: none");
    } else {
        println!("EXTRA package files ({}):", extra.len());
        for item in &extra {
            println!("  {item}");
        }
    }
    let data_root = package.join(&manifest.data_root);
    let validation = validate_root(&data_root)
        .map_err(|error| format!("cannot inspect {}: {error}", data_root.display()))?;
    if !validation.ok() {
        print_validation(&validation);
    }
    if missing.is_empty() && mismatched.is_empty() && extra.is_empty() && validation.ok() {
        println!(
            "PACKAGE OK: {} files verified, {} bytes",
            manifest.entries.len(),
            manifest.total_bytes()
        );
        Ok(())
    } else {
        Err("package verification failed".to_string())
    }
}

fn run_bench(
    root: &Path,
    save: Option<&Path>,
    event: u32,
    frames: usize,
    warmup: usize,
) -> Result<(), String> {
    let mut game = Game::new(root.to_path_buf())
        .map_err(|error| format!("Game::new({}): {error}", root.display()))?;
    if let Some(save) = save {
        game.load_save(save)
            .map_err(|error| format!("load_save({}): {error}", save.display()))?;
    }
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    if event != 0 {
        let _ = game.start_event(event);
    }
    for _ in 0..warmup {
        let _ = game.frame();
    }
    let report = measure_game(&mut game, frames);
    println!("{}", report.summary_line());
    println!(
        "  root={} save={} event={} warmup={} warnings={}",
        root.display(),
        save.map_or_else(|| "-".to_string(), |path| path.display().to_string()),
        event,
        warmup,
        game.stats.warnings
    );
    Ok(())
}

fn get_number(object: &[(String, Json)], key: &str) -> Result<i64, String> {
    object
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| value.as_number())
        .ok_or_else(|| format!("manifest has no numeric {key}"))
}

fn get_string<'a>(object: &'a [(String, Json)], key: &str) -> Result<&'a str, String> {
    object
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, value)| value.as_str())
        .ok_or_else(|| format!("manifest has no string {key}"))
}

fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(value) => Some(value),
            _ => None,
        }
    }
    fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(value) => Some(value),
            _ => None,
        }
    }
    fn as_number(&self) -> Option<i64> {
        match self {
            Json::Number(value) => Some(*value),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(value) => Some(value),
            _ => None,
        }
    }
}

fn parse_json(text: &str) -> Result<Json, String> {
    let mut parser = JsonParser {
        bytes: text.as_bytes(),
        position: 0,
    };
    let value = parser.value()?;
    parser.whitespace();
    if parser.position != parser.bytes.len() {
        return Err("trailing data after JSON value".to_string());
    }
    Ok(value)
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> JsonParser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.position += 1;
        Some(byte)
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.position += 1;
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.whitespace();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => {
                self.literal(b"true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(Json::Bool(false))
            }
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(Json::Null)
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("unexpected JSON byte at offset {}", self.position)),
        }
    }

    fn literal(&mut self, word: &[u8]) -> Result<(), String> {
        if self.bytes.get(self.position..self.position + word.len()) == Some(word) {
            self.position += word.len();
            Ok(())
        } else {
            Err(format!("bad JSON literal at offset {}", self.position))
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.bump();
        self.whitespace();
        let mut entries = Vec::new();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Json::Object(entries));
        }
        loop {
            self.whitespace();
            let key = self.string()?;
            self.whitespace();
            if self.bump() != Some(b':') {
                return Err(format!("expected colon at offset {}", self.position));
            }
            let value = self.value()?;
            entries.push((key, value));
            self.whitespace();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => {
                    return Err(format!(
                        "expected comma or brace at offset {}",
                        self.position
                    ))
                }
            }
        }
        Ok(Json::Object(entries))
    }

    fn array(&mut self) -> Result<Json, String> {
        self.bump();
        self.whitespace();
        let mut entries = Vec::new();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(Json::Array(entries));
        }
        loop {
            entries.push(self.value()?);
            self.whitespace();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => {
                    return Err(format!(
                        "expected comma or bracket at offset {}",
                        self.position
                    ))
                }
            }
        }
        Ok(Json::Array(entries))
    }

    fn string(&mut self) -> Result<String, String> {
        if self.bump() != Some(b'"') {
            return Err(format!("expected string at offset {}", self.position));
        }
        let mut out = String::new();
        while let Some(byte) = self.bump() {
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escaped = self
                        .bump()
                        .ok_or_else(|| "unfinished JSON escape".to_string())?;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self
                                .bytes
                                .get(self.position..self.position + 4)
                                .ok_or_else(|| "unfinished unicode escape".to_string())?;
                            let text = std::str::from_utf8(hex)
                                .map_err(|_| "bad unicode escape".to_string())?;
                            let code = u32::from_str_radix(text, 16)
                                .map_err(|_| "bad unicode escape".to_string())?;
                            self.position += 4;
                            let ch = char::from_u32(code)
                                .ok_or_else(|| "invalid unicode scalar".to_string())?;
                            out.push(ch);
                        }
                        _ => return Err("unsupported JSON escape".to_string()),
                    }
                }
                byte if byte < 0x20 => {
                    return Err("control byte in JSON string".to_string());
                }
                byte => out.push(byte as char),
            }
        }
        Err("unterminated JSON string".to_string())
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.position += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.position])
            .map_err(|_| "bad JSON number".to_string())?;
        let value = text
            .parse::<i64>()
            .map_err(|_| format!("bad JSON number {text}"))?;
        Ok(Json::Number(value))
    }
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    length: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffered: 0,
            length: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u64);
        if self.buffered > 0 {
            let take = (64 - self.buffered).min(data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }
        while data.len() >= 64 {
            let block: &[u8; 64] = data[..64].try_into().expect("64-byte block");
            self.compress(block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buffer[self.buffered..self.buffered + data.len()].copy_from_slice(data);
            self.buffered += data.len();
        }
    }

    fn finalize(mut self) -> [u8; 32] {
        let bit_length = self.length.wrapping_mul(8);
        let mut padding = vec![0x80u8];
        while (self.buffered + padding.len()) % 64 != 56 {
            padding.push(0);
        }
        padding.extend_from_slice(&bit_length.to_be_bytes());
        self.update(&padding);
        let mut out = [0u8; 32];
        for (index, word) in self.state.iter().enumerate() {
            out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut w = [0u32; 64];
        for index in 0..16 {
            let offset = index * 4;
            w[index] = u32::from_be_bytes([
                block[offset],
                block[offset + 1],
                block[offset + 2],
                block[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

#[cfg(test)]
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hash_file(path: &Path) -> Result<(u64, String), String> {
    let mut file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        hasher.update(&buffer[..read]);
    }
    let digest = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((total, digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn manifest_json_round_trips() {
        let manifest = Manifest {
            format: MANIFEST_FORMAT,
            tool: "wolf-package".to_string(),
            data_root: "data".to_string(),
            entries: vec![ManifestEntry {
                path: "data/BasicData/Game.dat".to_string(),
                bytes: 123,
                sha256: "00".repeat(32),
            }],
        };
        let parsed = Manifest::from_json(&manifest.to_json()).expect("parse");
        assert_eq!(parsed.format, manifest.format);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].path, "data/BasicData/Game.dat");
        assert_eq!(parsed.entries[0].bytes, 123);
    }
}
