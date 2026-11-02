//! Startup validation of the unpacked WOLF data root.
//!
//! The packer (`wolf-package validate`) and the game loader must agree on what a
//! playable root looks like. Before this module existed `Game::new` discovered a
//! missing root only when the first `read` failed, which produced a low-level
//! `cannot read ...: The system cannot find the file specified` message and no hint
//! about which directory should have been passed. The check here is deliberately
//! shallow (presence and non-zero size) because the packer performs the full tree
//! walk and hashing pass; the loader only needs to fail early and clearly.

use std::fmt;
use std::path::{Path, PathBuf};

/// Top-level directories a complete unpacked WOLF root carries.
pub const REQUIRED_DIRS: &[&str] = &["BasicData", "MapData", "SystemFile"];

/// The `.dat` files `Game::new` and `Databases::load` need before a frame can run.
///
/// Paths are relative to `BasicData/` when that directory exists, otherwise to the
/// root itself. Keeping this list in wolf-game lets `wolf-package` reuse it instead
/// of maintaining a second copy that can drift.
pub const REQUIRED_FILES: &[&str] = &[
    "Game.dat",
    "CommonEvent.dat",
    "CDataBase.dat",
    "DataBase.dat",
    "SysDatabase.dat",
    "SysDataBaseBasic.dat",
    "TileSetData.dat",
    "MapTree.dat",
];

/// Why a directory cannot be used as a game data root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataRootError {
    /// The path the caller passed.
    pub root: PathBuf,
    /// Human-readable missing or empty entries, already prefixed with their kind.
    pub problems: Vec<String>,
}

impl fmt::Display for DataRootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "data root {} is not a playable WOLF root: {}; expected BasicData/, MapData/, SystemFile/ (pass --root or set WOLF_DATA_ROOT)",
            self.root.display(),
            self.problems.join("; ")
        )
    }
}

impl std::error::Error for DataRootError {}

/// Checks the directory layout, returning every missing piece in one message.
///
/// This never panics and never touches the network. A flat layout (the eight `.dat`
/// files directly under `root`) is accepted for the diagnostics tools that unpack
/// only `BasicData.wolf`; the packer uses the stricter `BasicData/` layout.
pub fn check_layout(root: &Path) -> Result<(), DataRootError> {
    let mut problems = Vec::new();
    if !root.is_dir() {
        problems.push("root directory does not exist".to_string());
        return Err(DataRootError {
            root: root.to_path_buf(),
            problems,
        });
    }

    let has_basic_dir = root.join("BasicData").is_dir();
    let basic = if has_basic_dir {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };

    if !has_basic_dir {
        problems.push("missing BasicData/ directory (or flat .dat files in the root)".to_string());
    }
    for dir in ["MapData", "SystemFile"] {
        if !root.join(dir).is_dir() {
            problems.push(format!("missing {dir}/ directory"));
        }
    }
    for file in REQUIRED_FILES {
        let path = basic.join(file);
        if !path.is_file() {
            problems.push(format!("missing {}", path.display()));
        } else if path.metadata().map(|meta| meta.len()).unwrap_or(0) == 0 {
            problems.push(format!("empty {}", path.display()));
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(DataRootError {
            root: root.to_path_buf(),
            problems,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path.push("target");
        path.push("test-scratch");
        path.push(format!("data-root-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        path
    }

    #[test]
    fn a_missing_root_is_reported_as_a_layout_error() {
        let err = check_layout(Path::new("definitely-no-such-wolf-root")).expect_err("must fail");
        assert!(err.to_string().contains("root directory does not exist"));
    }

    #[test]
    fn missing_directories_and_files_are_listed_together() {
        let root = scratch("missing");
        std::fs::create_dir_all(root.join("BasicData")).expect("basic");
        std::fs::write(root.join("BasicData/Game.dat"), b"x").expect("game");
        let err = check_layout(&root).expect_err("incomplete");
        let text = err.to_string();
        assert!(text.contains("CommonEvent.dat"), "{text}");
        assert!(text.contains("missing MapData/"), "{text}");
        assert!(text.contains("missing SystemFile/"), "{text}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_complete_synthetic_layout_passes() {
        let root = scratch("complete");
        for dir in REQUIRED_DIRS {
            std::fs::create_dir_all(root.join(dir)).expect("dir");
        }
        for file in REQUIRED_FILES {
            std::fs::write(root.join("BasicData").join(file), b"x").expect("file");
        }
        check_layout(&root).expect("complete root");
        let _ = std::fs::remove_dir_all(&root);
    }
}
