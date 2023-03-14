//! Mount file systems
#[cfg(any(target_os = "android", target_os = "linux"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
mod linux;

#[cfg(any(target_os = "android", target_os = "linux"))]
pub use self::linux::*;

#[cfg(any(target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "openbsd"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
mod bsd;

#[cfg(any(target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "openbsd"
          ))]
pub use self::bsd::*;
