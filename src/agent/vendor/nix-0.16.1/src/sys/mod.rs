#[cfg(any(target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "linux",
          target_os = "macos",
          target_os = "netbsd"))]
pub mod aio;

#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod epoll;

#[cfg(any(target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "openbsd"))]
pub mod event;

#[cfg(target_os = "linux")]
pub mod eventfd;

#[cfg(any(target_os = "android",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "linux",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "openbsd"))]
#[macro_use]
pub mod ioctl;

#[cfg(target_os = "linux")]
pub mod memfd;

pub mod mman;

pub mod pthread;

#[cfg(any(target_os = "android",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "linux",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "openbsd"))]
pub mod ptrace;

#[cfg(target_os = "linux")]
pub mod quota;

#[cfg(any(target_os = "linux"))]
pub mod reboot;

pub mod select;

#[cfg(any(target_os = "android",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "linux",
          target_os = "macos"))]
pub mod sendfile;

pub mod signal;

#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod signalfd;

pub mod socket;

pub mod stat;

#[cfg(any(target_os = "android",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "linux",
          target_os = "macos",
          target_os = "openbsd"
))]
pub mod statfs;

pub mod statvfs;

#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod sysinfo;

pub mod termios;

pub mod time;

pub mod uio;

pub mod utsname;

pub mod wait;

#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod inotify;
