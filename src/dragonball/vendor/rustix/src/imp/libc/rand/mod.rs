mod types;

pub(crate) mod syscalls;

#[cfg(target_os = "linux")]
pub use types::GetRandomFlags;
