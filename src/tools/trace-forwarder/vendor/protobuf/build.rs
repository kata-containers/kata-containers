use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process;

// % rustc +stable --version
// rustc 1.26.0 (a77568041 2018-05-07)
// % rustc +beta --version
// rustc 1.27.0-beta.1 (03fb2f447 2018-05-09)
// % rustc +nightly --version
// rustc 1.27.0-nightly (acd3871ba 2018-05-10)
fn version_is_nightly(version: &str) -> bool {
    version.contains("nightly")
}

fn main() {
    let rustc = env::var("RUSTC").expect("RUSTC unset");

    let mut child = process::Command::new(rustc)
        .args(&["--version"])
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::piped())
        .spawn()
        .expect("spawn rustc");

    let mut rustc_version = String::new();

    child
        .stdout
        .as_mut()
        .expect("stdout")
        .read_to_string(&mut rustc_version)
        .expect("read_to_string");
    assert!(child.wait().expect("wait").success());

    if version_is_nightly(&rustc_version) {
        println!("cargo:rustc-cfg=rustc_nightly");
    }

    write_version();
}

fn out_dir() -> PathBuf {
    PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"))
}

fn version() -> String {
    env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION")
}

fn write_version() {
    let version = version();
    let version_ident = format!(
        "VERSION_{}",
        version.replace(".", "_").replace("-", "_").to_uppercase()
    );
    let mut file = File::create(Path::join(&out_dir(), "version.rs")).expect("open");
    writeln!(file, "/// protobuf crate version").unwrap();
    writeln!(file, "pub const VERSION: &'static str = \"{}\";", version).unwrap();
    writeln!(file, "/// This symbol is used by codegen").unwrap();
    writeln!(file, "#[doc(hidden)]").unwrap();
    writeln!(
        file,
        "pub const VERSION_IDENT: &'static str = \"{}\";",
        version_ident
    )
    .unwrap();
    writeln!(
        file,
        "/// This symbol can be referenced to assert that proper version of crate is used"
    )
    .unwrap();
    writeln!(file, "pub const {}: () = ();", version_ident).unwrap();
    file.flush().unwrap();
}
