use std::env::var;
use std::io::Write;

fn main() {
    // I/O safety is stabilized in Rust 1.63.
    if has_io_safety() {
        use_feature("io_safety_is_in_std")
    }

    // Work around
    // https://github.com/rust-lang/rust/issues/103306.
    use_feature_or_nothing("wasi_ext");

    // Rust 1.56 and earlier don't support panic in const fn.
    if has_panic_in_const_fn() {
        use_feature("panic_in_const_fn")
    }

    // Don't rerun this on changes other than build.rs, as we only depend on
    // the rustc version.
    println!("cargo:rerun-if-changed=build.rs");
}

fn use_feature_or_nothing(feature: &str) {
    if has_feature(feature) {
        use_feature(feature);
    }
}

fn use_feature(feature: &str) {
    println!("cargo:rustc-cfg={}", feature);
}

/// Test whether the rustc at `var("RUSTC")` supports the given feature.
fn has_feature(feature: &str) -> bool {
    let out_dir = var("OUT_DIR").unwrap();
    let rustc = var("RUSTC").unwrap();
    let target = var("TARGET").unwrap();

    let mut child = std::process::Command::new(rustc)
        .arg("--crate-type=rlib") // Don't require `main`.
        .arg("--emit=metadata") // Do as little as possible but still parse.
        .arg("--target")
        .arg(target)
        .arg("--out-dir")
        .arg(out_dir) // Put the output somewhere inconsequential.
        .arg("-") // Read from stdin.
        .stdin(std::process::Stdio::piped()) // Stdin is a pipe.
        .spawn()
        .unwrap();

    writeln!(child.stdin.take().unwrap(), "#![feature({})]", feature).unwrap();

    child.wait().unwrap().success()
}

/// Test whether the rustc at `var("RUSTC")` supports panic in `const fn`.
fn has_panic_in_const_fn() -> bool {
    let out_dir = var("OUT_DIR").unwrap();
    let rustc = var("RUSTC").unwrap();
    let target = var("TARGET").unwrap();

    let mut child = std::process::Command::new(rustc)
        .arg("--crate-type=rlib") // Don't require `main`.
        .arg("--emit=metadata") // Do as little as possible but still parse.
        .arg("--target")
        .arg(target)
        .arg("--out-dir")
        .arg(out_dir) // Put the output somewhere inconsequential.
        .arg("-") // Read from stdin.
        .stdin(std::process::Stdio::piped()) // Stdin is a pipe.
        .spawn()
        .unwrap();

    writeln!(child.stdin.take().unwrap(), "const fn foo() {{ panic!() }}").unwrap();

    child.wait().unwrap().success()
}

/// Test whether the rustc at `var("RUSTC")` supports the I/O safety feature.
fn has_io_safety() -> bool {
    let out_dir = var("OUT_DIR").unwrap();
    let rustc = var("RUSTC").unwrap();
    let target = var("TARGET").unwrap();

    let mut child = std::process::Command::new(rustc)
        .arg("--crate-type=rlib") // Don't require `main`.
        .arg("--emit=metadata") // Do as little as possible but still parse.
        .arg("--target")
        .arg(target)
        .arg("--out-dir")
        .arg(out_dir) // Put the output somewhere inconsequential.
        .arg("-") // Read from stdin.
        .stdin(std::process::Stdio::piped()) // Stdin is a pipe.
        .spawn()
        .unwrap();

    writeln!(
        child.stdin.take().unwrap(),
        "\
    #[cfg(unix)]\n\
    use std::os::unix::io::OwnedFd as Owned;\n\
    #[cfg(target_os = \"wasi\")]\n\
    use std::os::wasi::io::OwnedFd as Owned;\n\
    #[cfg(windows)]\n\
    use std::os::windows::io::OwnedHandle as Owned;\n\
    \n\
    pub type Success = Owned;\n\
    "
    )
    .unwrap();

    child.wait().unwrap().success()
}
