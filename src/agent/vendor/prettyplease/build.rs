fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    println!(concat!("cargo:VERSION=", env!("CARGO_PKG_VERSION")));
}
