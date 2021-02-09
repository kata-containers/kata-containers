//! Finds the appropriate `protoc` binary and Protobuf include directory for this host, and outputs
//! build directives so that the main `prost-build` crate can use them.
//!
//! The following locations are checked for `protoc` in decreasing priority:
//!
//!     1. The `PROTOC` environment variable.
//!     2. The bundled `protoc`.
//!     3. The `protoc` on the `PATH`.
//!
//! If no `protoc` binary is available in these locations, the build fails.
//!
//! The following locations are checked for the Protobuf include directory in decreasing priority:
//!
//!     1. The `PROTOC_INCLUDE` environment variable.
//!     2. The bundled Protobuf include directory.

use std::env;
use std::path::PathBuf;

/// Returns the path to the location of the bundled Protobuf artifacts.
fn bundle_path() -> PathBuf {
    env::current_dir()
        .unwrap()
        .join("third-party")
        .join("protobuf")
}

/// Returns the path to the `protoc` pointed to by the `PROTOC` environment variable, if it is set.
fn env_protoc() -> Option<PathBuf> {
    let protoc = match env::var_os("PROTOC") {
        Some(path) => PathBuf::from(path),
        None => return None,
    };

    if !protoc.exists() {
        panic!(
            "PROTOC environment variable points to non-existent file ({:?})",
            protoc
        );
    }

    Some(protoc)
}

/// Returns the path to the bundled `protoc`, if it is available for the host platform.
fn bundled_protoc() -> Option<PathBuf> {
    let protoc_bin_name = match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86") => "protoc-linux-x86_32",
        ("linux", "x86_64") => "protoc-linux-x86_64",
        ("linux", "aarch64") => "protoc-linux-aarch_64",
        ("macos", "x86_64") => "protoc-osx-x86_64",
        ("windows", _) => "protoc-win32.exe",
        _ => return None,
    };

    Some(bundle_path().join(protoc_bin_name))
}

/// Returns the path to the `protoc` included on the `PATH`, if it exists.
fn path_protoc() -> Option<PathBuf> {
    which::which("protoc").ok()
}

/// Returns the path to the Protobuf include directory pointed to by the `PROTOC_INCLUDE`
/// environment variable, if it is set.
fn env_protoc_include() -> Option<PathBuf> {
    let protoc_include = match env::var_os("PROTOC_INCLUDE") {
        Some(path) => PathBuf::from(path),
        None => return None,
    };

    if !protoc_include.exists() {
        panic!(
            "PROTOC_INCLUDE environment variable points to non-existent directory ({:?})",
            protoc_include
        );
    }
    if !protoc_include.is_dir() {
        panic!(
            "PROTOC_INCLUDE environment variable points to a non-directory file ({:?})",
            protoc_include
        );
    }

    Some(protoc_include)
}

/// Returns the path to the bundled Protobuf include directory.
fn bundled_protoc_include() -> PathBuf {
    bundle_path().join("include")
}

fn main() {
    let protoc = env_protoc()
        .or_else(bundled_protoc)
        .or_else(path_protoc)
        .expect(
            "Failed to find the protoc binary. The PROTOC environment variable is not set, \
             there is no bundled protoc for this platform, and protoc is not in the PATH",
        );

    let protoc_include = env_protoc_include().unwrap_or_else(bundled_protoc_include);

    println!("cargo:rustc-env=PROTOC={}", protoc.display());
    println!(
        "cargo:rustc-env=PROTOC_INCLUDE={}",
        protoc_include.display()
    );
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=PROTOC_INCLUDE");
}
