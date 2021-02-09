mod test_signal;

// NOTE: DragonFly lacks a kernel-level implementation of Posix AIO as of
// this writing. There is an user-level implementation, but whether aio
// works or not heavily depends on which pthread implementation is chosen
// by the user at link time. For this reason we do not want to run aio test
// cases on DragonFly.
#[cfg(any(target_os = "freebsd",
          target_os = "ios",
          target_os = "linux",
          target_os = "macos",
          target_os = "netbsd"))]
mod test_aio;
#[cfg(target_os = "linux")]
mod test_signalfd;
mod test_socket;
mod test_sockopt;
mod test_select;
#[cfg(any(target_os = "android", target_os = "linux"))]
mod test_sysinfo;
mod test_termios;
mod test_ioctl;
mod test_wait;
mod test_uio;

#[cfg(target_os = "linux")]
mod test_epoll;
#[cfg(target_os = "linux")]
mod test_inotify;
mod test_pthread;
#[cfg(any(target_os = "android",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "linux",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "openbsd"))]
mod test_ptrace;
