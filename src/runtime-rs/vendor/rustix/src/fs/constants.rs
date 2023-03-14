//! Filesystem API constants, translated into `bitflags` constants.

use crate::imp;

pub use imp::fs::FdFlags;

pub use imp::fs::Access;

#[cfg(not(target_os = "redox"))]
pub use imp::fs::AtFlags;

pub use imp::fs::Mode;

pub use imp::fs::OFlags;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use imp::fs::CloneFlags;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use imp::fs::CopyfileFlags;

#[cfg(any(target_os = "android", target_os = "linux"))]
pub use imp::fs::ResolveFlags;

#[cfg(any(target_os = "android", target_os = "linux"))]
pub use imp::fs::RenameFlags;
