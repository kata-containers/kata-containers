#![warn(rust_2018_idioms, single_use_lifetimes)]

use autocfg::AutoCfg;

// The rustc-cfg strings below are *not* public API. Please let us know by
// opening a GitHub issue if your build environment requires some way to enable
// these cfgs other than by executing our build script.
fn main() {
    let cfg = match AutoCfg::new() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!(
                "cargo:warning={}: unable to determine rustc version: {}",
                env!("CARGO_PKG_NAME"),
                e
            );
            return;
        }
    };

    // Function like procedural macros in expressions patterns statements stabilized in Rust 1.45:
    // https://blog.rust-lang.org/2020/07/16/Rust-1.45.0.html#stabilizing-function-like-procedural-macros-in-expressions-patterns-and-statements
    if cfg.probe_rustc_version(1, 45) {
        println!("cargo:rustc-cfg=fn_like_proc_macro");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
