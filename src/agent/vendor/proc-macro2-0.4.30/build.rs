// rustc-cfg emitted by the build script:
//
// "u128"
//     Include u128 and i128 constructors for proc_macro2::Literal. Enabled on
//     any compiler 1.26+.
//
// "use_proc_macro"
//     Link to extern crate proc_macro. Available on any compiler and any target
//     except wasm32. Requires "proc-macro" Cargo cfg to be enabled (default is
//     enabled). On wasm32 we never link to proc_macro even if "proc-macro" cfg
//     is enabled.
//
// "wrap_proc_macro"
//     Wrap types from libproc_macro rather than polyfilling the whole API.
//     Enabled on rustc 1.29+ as long as procmacro2_semver_exempt is not set,
//     because we can't emulate the unstable API without emulating everything
//     else. Also enabled unconditionally on nightly, in which case the
//     procmacro2_semver_exempt surface area is implemented by using the
//     nightly-only proc_macro API.
//
// "slow_extend"
//     Fallback when `impl Extend for TokenStream` is not available. These impls
//     were added one version later than the rest of the proc_macro token API.
//     Enabled on rustc 1.29 only.
//
// "proc_macro_span"
//     Enable non-dummy behavior of Span::start and Span::end methods which
//     requires an unstable compiler feature. Enabled when building with
//     nightly, unless `-Z allow-feature` in RUSTFLAGS disallows unstable
//     features.
//
// "super_unstable"
//     Implement the semver exempt API in terms of the nightly-only proc_macro
//     API. Enabled when using procmacro2_semver_exempt on a nightly compiler.
//
// "span_locations"
//     Provide methods Span::start and Span::end which give the line/column
//     location of a token. Enabled by procmacro2_semver_exempt or the
//     "span-locations" Cargo cfg. This is behind a cfg because tracking
//     location inside spans is a performance hit.

use std::env;
use std::process::Command;
use std::str;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target = env::var("TARGET").unwrap();

    let version = match rustc_version() {
        Some(version) => version,
        None => return,
    };

    if version.minor >= 26 {
        println!("cargo:rustc-cfg=u128");
    }

    let semver_exempt = cfg!(procmacro2_semver_exempt);
    if semver_exempt {
        // https://github.com/alexcrichton/proc-macro2/issues/147
        println!("cargo:rustc-cfg=procmacro2_semver_exempt");
    }

    if semver_exempt || cfg!(feature = "span-locations") {
        println!("cargo:rustc-cfg=span_locations");
    }

    if !enable_use_proc_macro(&target) {
        return;
    }

    println!("cargo:rustc-cfg=use_proc_macro");

    // Rust 1.29 stabilized the necessary APIs in the `proc_macro` crate
    if version.nightly || version.minor >= 29 && !semver_exempt {
        println!("cargo:rustc-cfg=wrap_proc_macro");
    }

    if version.minor == 29 {
        println!("cargo:rustc-cfg=slow_extend");
    }

    if version.nightly && feature_allowed("proc_macro_span") {
        println!("cargo:rustc-cfg=proc_macro_span");
    }

    if semver_exempt && version.nightly {
        println!("cargo:rustc-cfg=super_unstable");
    }
}

fn enable_use_proc_macro(target: &str) -> bool {
    // wasm targets don't have the `proc_macro` crate, disable this feature.
    if target.contains("wasm32") {
        return false;
    }

    // Otherwise, only enable it if our feature is actually enabled.
    cfg!(feature = "proc-macro")
}

struct RustcVersion {
    minor: u32,
    nightly: bool,
}

fn rustc_version() -> Option<RustcVersion> {
    macro_rules! otry {
        ($e:expr) => {
            match $e {
                Some(e) => e,
                None => return None,
            }
        };
    }

    let rustc = otry!(env::var_os("RUSTC"));
    let output = otry!(Command::new(rustc).arg("--version").output().ok());
    let version = otry!(str::from_utf8(&output.stdout).ok());
    let nightly = version.contains("nightly");
    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }
    let minor = otry!(pieces.next());
    let minor = otry!(minor.parse().ok());

    Some(RustcVersion {
        minor: minor,
        nightly: nightly,
    })
}

fn feature_allowed(feature: &str) -> bool {
    // Recognized formats:
    //
    //     -Z allow-features=feature1,feature2
    //
    //     -Zallow-features=feature1,feature2

    if let Some(rustflags) = env::var_os("RUSTFLAGS") {
        for mut flag in rustflags.to_string_lossy().split(' ') {
            if flag.starts_with("-Z") {
                flag = &flag["-Z".len()..];
            }
            if flag.starts_with("allow-features=") {
                flag = &flag["allow-features=".len()..];
                return flag.split(',').any(|allowed| allowed == feature);
            }
        }
    }

    // No allow-features= flag, allowed by default.
    true
}
