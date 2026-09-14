//! Reads every fixture and writes it back out, for the C# side to read.
//!
//! usage: cargo run -p tc-core --example rewrite -- <output dir>

use tc_core::Tour;

fn main() {
    let out = std::env::args().nth(1).expect("output directory");
    std::fs::create_dir_all(&out).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");

    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(name) = file.strip_suffix(".tour.json") else {
            continue;
        };
        let tour = Tour::from_json(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
        let path = std::path::Path::new(&out).join(format!("{name}.tour.json"));
        std::fs::write(&path, tour.to_json().unwrap()).unwrap();
        println!("{name}");
    }
}
