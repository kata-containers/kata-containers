use std::env;
use std::process::Command;
use std::str;

// FIXME: replace it with '?' operator
macro_rules! try_opt {
    ($e:expr) => {
        match { $e } {
            Some(e) => e,
            None => return None,
        }
    };
}

fn rustc_minor_version() -> Option<u32> {
    let rustc = try_opt!(env::var_os("RUSTC"));
    let output = try_opt!(Command::new(rustc).arg("--version").output().ok());
    let version_str = try_opt!(str::from_utf8(&output.stdout).ok());
    let minor_str = try_opt!(version_str.split('.').nth(1));

    minor_str.parse().ok()
}

fn main() {
    let minor = match rustc_minor_version() {
        Some(m) => m,
        None => return,
    };

    let target = env::var("TARGET").unwrap();
    let is_emscripten = target == "asmjs-unknown-emscripten"
        || target == "wasm32-unknown-emscripten";

    if minor >= 26 && !is_emscripten {
        println!("cargo:rustc-cfg=integer128");
    }

    // workaround on macro bugs fixed in 1.20
    //
    // https://github.com/rust-lang/rust/pull/42913
    if minor < 20 {
        println!("cargo:rustc-cfg=macro_workaround");
    }
}
