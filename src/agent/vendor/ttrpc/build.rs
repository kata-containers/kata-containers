use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let path: PathBuf = [out_dir.clone(), "mod.rs".to_string()].iter().collect();
    fs::write(path, "pub mod ttrpc;").unwrap();

    protobuf_codegen_pure::Codegen::new()
        .out_dir(out_dir)
        .inputs(&["src/ttrpc.proto"])
        .include("src")
        .run()
        .expect("Codegen failed.");
}
