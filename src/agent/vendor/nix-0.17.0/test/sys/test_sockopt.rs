use rand::{thread_rng, Rng};
use nix::sys::socket::{socket, sockopt, getsockopt, setsockopt, AddressFamily, SockType, SockFlag, SockProtocol};

#[cfg(target_os = "linux")]
#[test]
fn is_so_mark_functional() {
    use nix::sys::socket::sockopt;

    require_capability!(CAP_NET_ADMIN);

    let s = socket(AddressFamily::Inet, SockType::Stream, SockFlag::empty(), None).unwrap();
    setsockopt(s, sockopt::Mark, &1337).unwrap();
    let mark = getsockopt(s, sockopt::Mark).unwrap();
    assert_eq!(mark, 1337);
}

#[test]
fn test_so_buf() {
    let fd = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(), SockProtocol::Udp)
             .unwrap();
    let bufsize: usize = thread_rng().gen_range(4096, 131_072);
    setsockopt(fd, sockopt::SndBuf, &bufsize).unwrap();
    let actual = getsockopt(fd, sockopt::SndBuf).unwrap();
    assert!(actual >= bufsize);
    setsockopt(fd, sockopt::RcvBuf, &bufsize).unwrap();
    let actual = getsockopt(fd, sockopt::RcvBuf).unwrap();
    assert!(actual >= bufsize);
}

// The CI doesn't supported getsockopt and setsockopt on emulated processors.
// It's beleived that a QEMU issue, the tests run ok on a fully emulated system.
// Current CI just run the binary with QEMU but the Kernel remains the same as the host.
// So the syscall doesn't work properly unless the kernel is also emulated.
#[test]
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    any(target_os = "freebsd", target_os = "linux")
))]
fn test_tcp_congestion() {
    use std::ffi::OsString;

    let fd = socket(AddressFamily::Inet, SockType::Stream, SockFlag::empty(), None).unwrap();

    let val = getsockopt(fd, sockopt::TcpCongestion).unwrap();
    setsockopt(fd, sockopt::TcpCongestion, &val).unwrap();

    setsockopt(fd, sockopt::TcpCongestion, &OsString::from("tcp_congestion_does_not_exist")).unwrap_err();

    assert_eq!(
        getsockopt(fd, sockopt::TcpCongestion).unwrap(),
        val
    );
}
