#[cfg(all(feature = "snapshot-unionfs", not(target_arch = "x86_64")))]
compile_error!("occlum only available on Intel x86_64");

pub mod unionfs;
