use std::env;
use std::process::Command;
use std::str::FromStr;

fn main() {
    let minor = match rustc_minor_version() {
        Some(minor) => minor,
        None => return,
    };
    if minor >= 22 {
        println!("cargo:rustc-cfg=derive_copy");
    }
    if minor >= 28 {
        println!("cargo:rustc-cfg=repr_transparent");
    }
    if minor >= 36 {
        println!("cargo:rustc-cfg=native_uninit");
    }

}

fn rustc_minor_version() -> Option<u32> {
    let rustc = env::var_os("RUSTC");

    let output = rustc.and_then(|rustc| {
        Command::new(rustc).arg("--version").output().ok()
    });

    let version = output.and_then(|output| {
        String::from_utf8(output.stdout).ok()
    });

    let version = if let Some(version) = version {
        version
    } else {
        return None;
    };

    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }

    let next = match pieces.next() {
        Some(next) => next,
        None => return None,
    };

    u32::from_str(next).ok()
}
