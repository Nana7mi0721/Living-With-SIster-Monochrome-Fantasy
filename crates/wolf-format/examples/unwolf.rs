use std::path::{Path, PathBuf};

use wolf_format::dxarchive::DxArchive;

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "list".into());
    let archive_path = args.next().expect("archive path");
    let out_dir: Option<PathBuf> = args.next().map(PathBuf::from);
    let data = std::fs::read(&archive_path).expect("read archive");
    let archive = DxArchive::open(&data).expect("parse archive");
    println!("version=8 entries={}", archive.entries().len());
    for entry in archive.entries() {
        let rel = if entry.directory.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", entry.directory, entry.name)
        };
        println!("{rel}\t{}", entry.size);
        if mode == "extract" {
            let out = match &out_dir {
                Some(dir) => dir.join(Path::new(&rel)),
                None => PathBuf::from(&rel),
            };
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            let bytes = archive.read(entry).expect("read entry");
            std::fs::write(&out, &bytes).expect("write entry");
        }
    }
}
