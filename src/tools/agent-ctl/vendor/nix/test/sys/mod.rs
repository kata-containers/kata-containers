mod test_signal;

// NOTE: DragonFly lacks a kernel-level implementation of Posix AIO as of
// this writing. There is an user-level implementation, but whether aio
// works or not heavily depends on which pthread implementation is chosen
// by the user at link time. For this reason we do not want to run aio test
// cases on DragonFly.
#[cfg(any(
    target_os = "freebsd",
    target_os = "ios",
    all(target_os = "linux", not(target_env = "uclibc")),
    target_os = "macos",
    target_os = "netbsd"
))]
mod test_aio;
#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku"
)))]
mod test_ioctl;
#[cfg(not(target_os = "redox"))]
mod test_mman;
#[cfg(not(target_os = "redox"))]
mod test_select;
#[cfg(target_os = "linux")]
mod test_signalfd;
#[cfg(not(any(target_os = "redox", target_os = "haiku")))]
mod test_socket;
#[cfg(not(any(target_os = "redox")))]
mod test_sockopt;
mod test_stat;
#[cfg(any(target_os = "android", target_os = "linux"))]
mod test_sysinfo;
#[cfg(not(any(
    target_os = "redox",
    target_os = "fuchsia",
    target_os = "haiku"
)))]
mod test_termios;
mod test_uio;
mod test_wait;

#[cfg(any(target_os = "android", target_os = "linux"))]
mod test_epoll;
#[cfg(target_os = "linux")]
mod test_inotify;
mod test_pthread;
#[cfg(any(
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod test_ptrace;
#[cfg(any(target_os = "android", target_os = "linux"))]
mod test_timerfd;
