#[cfg(target_os = "dragonfly")]
extern crate cc;

#[cfg(target_os = "dragonfly")]
fn main() {
    cc::Build::new()
        .file("src/errno_dragonfly.c")
        .compile("liberrno_dragonfly.a");
}

#[cfg(not(target_os = "dragonfly"))]
fn main() {}
