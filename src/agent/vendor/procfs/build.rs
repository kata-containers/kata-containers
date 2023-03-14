fn main() {
    // Filters are extracted from `libc` filters
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("Missing CARGO_CFG_TARGET_OS envvar");
    if !["android", "linux", "l4re"].contains(&target_os.as_str()) {
        eprintln!("Building procfs on an for a unsupported platform. Currently only linux and android are supported");
        eprintln!("(Your current target_os is {})", target_os);
        std::process::exit(1)
    }
}
