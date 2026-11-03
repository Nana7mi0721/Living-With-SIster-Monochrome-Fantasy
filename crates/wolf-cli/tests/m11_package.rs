//! M11-PKG black-box tests.
//!
//! These tests spawn the standalone wolf-package binary, so they exercise the
//! real command line and exit codes. The real-data test is skipped (with a printed
//! notice, not a failure) when work/all is absent; all packaging tests build a
//! tiny synthetic data root under work/out and never touch work/all.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn package_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wolf-package"))
}

fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        workspace_root().join(path)
    }
}

fn save01() -> PathBuf {
    workspace_root().join("work/retail_backup/Save/SaveData01.sav")
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(tag: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let base = workspace_root().join("work/out");
        fs::create_dir_all(&base).expect("create work/out");
        let path = base.join(format!("wolf-package-{tag}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, bytes).expect("write fixture file");
}

fn make_synthetic_root(root: &Path, complete: bool) {
    for dir in ["BasicData", "MapData", "SystemFile"] {
        fs::create_dir_all(root.join(dir)).expect("required dir");
    }
    for file in REQUIRED_FILES {
        if !complete && *file == "BasicData/CommonEvent.dat" {
            continue;
        }
        write_file(
            &root.join(file),
            format!("synthetic payload for {file}").as_bytes(),
        );
    }
    write_file(&root.join("MapData/TestMap.mps"), b"synthetic map");
    write_file(
        &root.join("SystemFile/TestWindow.png"),
        b"synthetic graphic",
    );
    // Retail data carries Japanese file names throughout; the manifest parser must
    // preserve them byte-for-byte or check reports every such file as missing + extra.
    write_file(
        &root.join("SystemFile/くしゃくしゃ.png"),
        b"synthetic non-ascii graphic",
    );
}

fn run_package(args: &[&str]) -> Output {
    Command::new(package_bin())
        .args(args)
        .output()
        .expect("failed to spawn wolf-package")
}

fn combined(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn validate_accepts_the_real_work_all_root() {
    let root = data_root();
    if !root.is_dir() {
        eprintln!("SKIP: data root {} is absent", root.display());
        return;
    }
    let output = run_package(&["validate", "--root", root.to_str().expect("utf8 path")]);
    let text = combined(&output);
    assert!(output.status.success(), "validate failed:\n{text}");
    assert!(text.contains("required dirs : 3/3 present"), "{text}");
    assert!(text.contains("required files: 8/8 present"), "{text}");
    assert!(text.contains("extra top-level: none"), "{text}");
    assert!(text.contains("VALID: data root is complete"), "{text}");
}

#[test]
fn validate_rejects_a_broken_root_and_lists_missing_and_extra() {
    let temp = TempDir::new("broken");
    let root = temp.path().join("root");
    make_synthetic_root(&root, false);
    write_file(&root.join("unexpected.txt"), b"not part of a WOLF root");

    let output = run_package(&["validate", "--root", root.to_str().expect("utf8 path")]);
    let text = combined(&output);
    assert!(
        !output.status.success(),
        "broken root was accepted:\n{text}"
    );
    assert!(text.contains("MISSING"), "{text}");
    assert!(text.contains("BasicData/CommonEvent.dat"), "{text}");
    assert!(text.contains("EXTRA top-level"), "{text}");
    assert!(text.contains("unexpected.txt"), "{text}");
}

#[test]
fn assemble_and_check_round_trip_the_manifest() {
    let temp = TempDir::new("assemble");
    let root = temp.path().join("root");
    make_synthetic_root(&root, true);
    // A real data root carries the .wolf build archives beside the unpacked tree;
    // packaging must skip them or it doubles the payload with unreachable bytes.
    write_file(&root.join("BasicData.wolf"), b"synthetic archive");
    let font = temp.path().join("Lolita.ttf");
    write_file(&font, b"synthetic font");
    let exe = temp.path().join("wolf-cli-source.exe");
    write_file(&exe, b"MZ synthetic executable");
    let runtime = temp.path().join("vcruntime140.dll");
    write_file(&runtime, b"synthetic runtime dll");
    let out = temp.path().join("package-a");

    let output = run_package(&[
        "assemble",
        "--root",
        root.to_str().expect("utf8 path"),
        "--exe",
        exe.to_str().expect("utf8 path"),
        "--out",
        out.to_str().expect("utf8 path"),
        "--runtime",
        runtime.to_str().expect("utf8 path"),
        "--font",
        font.to_str().expect("utf8 path"),
    ]);
    let text = combined(&output);
    assert!(output.status.success(), "assemble failed:\n{text}");
    assert!(text.contains("ASSEMBLED:"), "{text}");
    let manifest_path = out.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).expect("manifest exists");
    assert!(manifest.contains("\"sha256\""), "{manifest}");
    assert!(manifest.contains("\"bytes\""), "{manifest}");
    assert!(manifest.contains("data/BasicData/Game.dat"), "{manifest}");
    assert!(manifest.contains("wolf-cli.exe"), "{manifest}");
    assert!(manifest.contains("vcruntime140.dll"), "{manifest}");
    assert!(manifest.contains("data/Fonts/Lolita.ttf"), "{manifest}");
    assert!(
        manifest.contains("data/SystemFile/くしゃくしゃ.png"),
        "non-ASCII path must survive manifest serialization: {manifest}"
    );
    assert!(
        !manifest.contains(".wolf"),
        "archives must not be packaged: {manifest}"
    );

    let check = run_package(&["check", "--package", out.to_str().expect("utf8 path")]);
    let check_text = combined(&check);
    assert!(check.status.success(), "check failed:\n{check_text}");
    assert!(check_text.contains("PACKAGE OK"), "{check_text}");

    let out2 = temp.path().join("package-b");
    let output2 = run_package(&[
        "assemble",
        "--root",
        root.to_str().expect("utf8 path"),
        "--exe",
        exe.to_str().expect("utf8 path"),
        "--out",
        out2.to_str().expect("utf8 path"),
        "--runtime",
        runtime.to_str().expect("utf8 path"),
        "--font",
        font.to_str().expect("utf8 path"),
    ]);
    assert!(output2.status.success(), "{}", combined(&output2));
    let manifest2 = fs::read_to_string(out2.join("manifest.json")).expect("manifest2");
    assert_eq!(manifest, manifest2, "manifest must be deterministic");

    let mut tampered = fs::read(out.join("data/BasicData/Game.dat")).expect("read data");
    if let Some(first) = tampered.first_mut() {
        *first = b'X';
    }
    fs::write(out.join("data/BasicData/Game.dat"), tampered).expect("tamper");
    let bad = run_package(&["--check", out.to_str().expect("utf8 path")]);
    let bad_text = combined(&bad);
    assert!(
        !bad.status.success(),
        "tampered package was accepted:\n{bad_text}"
    );
    assert!(bad_text.contains("MISMATCHED"), "{bad_text}");
    assert!(bad_text.contains("sha256 mismatch"), "{bad_text}");

    fs::remove_file(out2.join("data/SystemFile/TestWindow.png")).expect("remove packaged file");
    let missing = run_package(&["check", "--package", out2.to_str().expect("utf8 path")]);
    let missing_text = combined(&missing);
    assert!(
        !missing.status.success(),
        "incomplete package was accepted:\n{missing_text}"
    );
    assert!(missing_text.contains("MISSING"), "{missing_text}");
    assert!(
        missing_text.contains("data/SystemFile/TestWindow.png"),
        "{missing_text}"
    );
}

fn token_f64(text: &str, key: &str) -> Option<f64> {
    text.split_whitespace()
        .find_map(|token| token.strip_prefix(key)?.parse().ok())
}

#[test]
fn frame_budget_benchmarks_a_real_game_loop() {
    let root = data_root();
    if !root.is_dir() {
        eprintln!("SKIP: data root {} is absent", root.display());
        return;
    }
    let save = save01();
    let mut args: Vec<String> = vec![
        "bench".into(),
        "--root".into(),
        root.display().to_string(),
        "--event".into(),
        "188".into(),
        "--frames".into(),
        "60".into(),
        "--warmup".into(),
        "5".into(),
    ];
    if save.is_file() {
        args.push("--save".into());
        args.push(save.display().to_string());
    }
    let output = Command::new(package_bin())
        .args(&args)
        .output()
        .expect("spawn wolf-package bench");
    let text = combined(&output);
    assert!(output.status.success(), "bench failed:\n{text}");
    assert!(text.contains("frame-budget: frames=60"), "{text}");
    let p50 = token_f64(&text, "p50_us=").expect("p50 present");
    let p95 = token_f64(&text, "p95_us=").expect("p95 present");
    let max = token_f64(&text, "max_us=").expect("max present");
    assert!(p50 >= 0.0, "{text}");
    assert!(p50 <= p95, "p50 {p50} > p95 {p95}: {text}");
    assert!(p95 <= max, "p95 {p95} > max {max}: {text}");
    assert!(max > 0.0, "{text}");
}
