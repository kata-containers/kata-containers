extern crate autocfg;

use std::env;

fn main() {
    let ac = autocfg::new();
    if ac.probe_type("i128") {
        println!("cargo:rustc-cfg=has_i128");
    } else if env::var_os("CARGO_FEATURE_I128").is_some() {
        panic!("i128 support was not detected!");
    }
    ac.emit_expression_cfg(
        "unsafe { 1f64.to_int_unchecked::<i32>() }",
        "has_to_int_unchecked",
    );

    autocfg::rerun_path("build.rs");
}
