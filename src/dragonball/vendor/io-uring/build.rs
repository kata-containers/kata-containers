#[cfg(not(feature = "bindgen"))]
fn main() {}

#[cfg(feature = "bindgen")]
fn main() {
    use std::env;
    use std::path::PathBuf;

    const INCLUDE: &str = r#"
#include <unistd.h>
#include <sys/syscall.h>
#include <linux/time_types.h>
#include <linux/stat.h>
#include <linux/openat2.h>
#include <linux/io_uring.h>
    "#;

    #[cfg(not(feature = "overwrite"))]
    let outdir = PathBuf::from(env::var("OUT_DIR").unwrap());

    #[cfg(feature = "overwrite")]
    let outdir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src/sys");

    bindgen::Builder::default()
        .header_contents("include-file.h", INCLUDE)
        .ctypes_prefix("libc")
        .prepend_enum_name(false)
        .derive_default(true)
        .generate_comments(true)
        .use_core()
        .allowlist_type("io_uring_.*|io_.qring_.*|__kernel_timespec|open_how")
        .allowlist_var("__NR_io_uring.*|IOSQE_.*|IORING_.*|IO_URING_.*|SPLICE_F_FD_IN_FIXED")
        .generate()
        .unwrap()
        .write_to_file(outdir.join("sys.rs"))
        .unwrap();
}
