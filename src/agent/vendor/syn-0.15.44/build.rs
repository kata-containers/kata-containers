use std::env;
use std::process::Command;
use std::str::{self, FromStr};

// The rustc-cfg strings below are *not* public API. Please let us know by
// opening a GitHub issue if your build environment requires some way to enable
// these cfgs other than by executing our build script.
fn main() {
    let compiler = match rustc_version() {
        Some(compiler) => compiler,
        None => return,
    };

    if compiler.minor >= 17 {
        println!("cargo:rustc-cfg=syn_can_match_trailing_dollar");
    }

    if compiler.minor >= 19 {
        println!("cargo:rustc-cfg=syn_can_use_thread_id");
    }

    if compiler.minor >= 20 {
        println!("cargo:rustc-cfg=syn_can_use_associated_constants");
    }

    // Macro modularization allows re-exporting the `quote!` macro in 1.30+.
    if compiler.minor >= 30 {
        println!("cargo:rustc-cfg=syn_can_call_macro_by_path");
    }

    if !compiler.nightly {
        println!("cargo:rustc-cfg=syn_disable_nightly_tests");
    }
}

struct Compiler {
    minor: u32,
    nightly: bool,
}

fn rustc_version() -> Option<Compiler> {
    let rustc = match env::var_os("RUSTC") {
        Some(rustc) => rustc,
        None => return None,
    };

    let output = match Command::new(rustc).arg("--version").output() {
        Ok(output) => output,
        Err(_) => return None,
    };

    let version = match str::from_utf8(&output.stdout) {
        Ok(version) => version,
        Err(_) => return None,
    };

    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }

    let next = match pieces.next() {
        Some(next) => next,
        None => return None,
    };

    let minor = match u32::from_str(next) {
        Ok(minor) => minor,
        Err(_) => return None,
    };

    Some(Compiler {
        minor: minor,
        nightly: version.contains("nightly"),
    })
}
