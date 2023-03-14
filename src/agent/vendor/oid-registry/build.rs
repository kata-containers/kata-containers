use std::env;

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/load.rs"));

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=assets/oid_db.txt");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("oid_db.rs");

    let m = load_file("assets/oid_db.txt")?;
    generate_file(&m, dest_path)?;

    Ok(())
}
