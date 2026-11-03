//! Embeds the Windows icon into `wolf-cli.exe`.
//!
//! The icon lives in `assets/wolf-cli.ico` (generated from the game's own
//! `Data/SystemFile/charicon.png`). Cargo has no built-in Windows resource support, so
//! this script writes a one-line `.rc` file and compiles it with the Windows SDK's
//! `rc.exe`. The resulting `.res` is handed to the linker for the `wolf-cli` binary only.
//!
//! `rc.exe` is not normally on `PATH` outside a Developer Command Prompt, so the
//! discovery below checks `RC`, `PATH`, `WindowsSdkDir` and finally the installed
//! Windows Kits versions. If it cannot be found the build still succeeds with a warning
//! (a missing icon must never break a non-Windows or minimal toolchain build).

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/wolf-cli.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set"),
    );
    let icon = manifest_dir.join("assets").join("wolf-cli.ico");
    if !icon.is_file() {
        println!(
            "cargo:warning=wolf-cli icon not found at {}; building without a program icon",
            icon.display()
        );
        return;
    }

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is always set"));
    let rc_file = out_dir.join("wolf-cli.rc");
    let res_file = out_dir.join("wolf-cli.res");
    // The resource compiler wants backslashes doubled inside a quoted string.
    let icon_literal = icon.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(&rc_file, format!("1 ICON \"{icon_literal}\"\n"))
        .expect("write generated .rc file");

    let Some(rc) = find_rc() else {
        println!(
            "cargo:warning=rc.exe not found; wolf-cli.exe will not carry its program icon"
        );
        return;
    };

    let status = Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_file)
        .arg(&rc_file)
        .status();
    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg-bin=wolf-cli={}", res_file.display());
        }
        Ok(status) => println!(
            "cargo:warning={} failed ({}); wolf-cli.exe will not carry its program icon",
            rc.display(),
            status
        ),
        Err(err) => println!(
            "cargo:warning=could not run {}: {err}; wolf-cli.exe will not carry its program icon",
            rc.display()
        ),
    }
}

fn find_rc() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("RC") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86") => "x86",
        _ => "x64",
    };

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(sdk) = std::env::var_os("WindowsSdkDir") {
        roots.push(PathBuf::from(sdk).join("bin"));
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(
            PathBuf::from(program_files)
                .join("Windows Kits")
                .join("10")
                .join("bin"),
        );
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        roots.push(
            PathBuf::from(program_files)
                .join("Windows Kits")
                .join("10")
                .join("bin"),
        );
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin"));

    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut versions: Vec<(Vec<u64>, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let key: Vec<u64> = name.split('.').filter_map(|part| part.parse().ok()).collect();
                if key.len() == 4 {
                    Some((key, entry.path()))
                } else {
                    None
                }
            })
            .collect();
        versions.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, version_dir) in versions {
            let candidate = version_dir.join(arch).join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}
