extern crate version_check;

use std::env;
use version_check::is_min_version;

fn main() {
    // Switch on for versions that have Error::source
    // As introduced by https://github.com/rust-lang/rust/pull/53533
    if is_min_version("1.30").unwrap_or(false) {
        println!("cargo:rustc-cfg=has_error_source");
    }

    if is_min_version("1.42").unwrap_or(false) {
        println!("cargo:rustc-cfg=has_error_description_deprecated");
    }

    // So we can get the build profile for has_backtrace_depending_on_env test
    if let Ok(profile) = env::var("PROFILE") {
        println!("cargo:rustc-cfg=build={:?}", profile);
    }
}
