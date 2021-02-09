//! This build script detects target platforms that lack proper support for
//! atomics and sets `cfg` flags accordingly.

use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();

    if !target.starts_with("thumbv6") {
        println!("cargo:rustc-cfg=atomic_cas");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
