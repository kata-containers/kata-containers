#![warn(rust_2018_idioms, single_use_lifetimes)]

use autocfg::AutoCfg;
use std::env;

include!("no_atomic_cas.rs");

// The rustc-cfg listed below are considered public API, but it is *unstable*
// and outside of the normal semver guarantees:
//
// - `futures_no_atomic_cas`
//      Assume the target does *not* support atomic CAS operations.
//      This is usually detected automatically by the build script, but you may
//      need to enable it manually when building for custom targets or using
//      non-cargo build systems that don't run the build script.
//
// With the exceptions mentioned above, the rustc-cfg strings below are
// *not* public API. Please let us know by opening a GitHub issue if your build
// environment requires some way to enable these cfgs other than by executing
// our build script.
fn main() {
    let target = match env::var("TARGET") {
        Ok(target) => target,
        Err(e) => {
            println!(
                "cargo:warning={}: unable to get TARGET environment variable: {}",
                env!("CARGO_PKG_NAME"),
                e
            );
            return;
        }
    };

    // Note that this is `no_*`, not `has_*`. This allows treating
    // `cfg(target_has_atomic = "ptr")` as true when the build script doesn't
    // run. This is needed for compatibility with non-cargo build systems that
    // don't run the build script.
    if NO_ATOMIC_CAS_TARGETS.contains(&&*target) {
        println!("cargo:rustc-cfg=futures_no_atomic_cas");
    }

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

    println!("cargo:rerun-if-changed=no_atomic_cas.rs");
}
