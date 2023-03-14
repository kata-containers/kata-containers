use std::fs::{self, DirEntry};
use x509_parser::parse_x509_certificate;

const ARTIFACTS_DIR: &str = "fuzz/artifacts/fuzzer_script_1";
const CORPUS_DIR: &str = "fuzz/corpus/fuzzer_script_1";

#[test]
fn run_all_fuzz_files() {
    parse_dir(ARTIFACTS_DIR);
    parse_dir(CORPUS_DIR);
}

fn parse_dir(name: &str) {
    match fs::read_dir(name) {
        Ok(dir_entries) => {
            dir_entries.for_each(|entry| {
                let _ = entry.as_ref().map(parse_file);
            });
        }
        Err(_) => eprintln!("fuzzer corpus/artifacts not found - ignoring test"),
    }
}

fn parse_file(entry: &DirEntry) -> std::io::Result<()> {
    let path = entry.path();
    // println!("{:?}", entry.path());
    let data = fs::read(path).unwrap();
    let _ = parse_x509_certificate(&data);
    Ok(())
}

#[test]
#[ignore = "placeholder for specific tests"]
fn run_fuzz_candidate() {
    const CANDIDATE: &str = "fuzz/corpus/fuzzer_script_1/bd0096a63b9979d64763915a342a59af9dc281fb";

    let data = fs::read(CANDIDATE).unwrap();
    let _ = parse_x509_certificate(&data);
}
