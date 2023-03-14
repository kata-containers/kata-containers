#[test]
fn test_error() {
    assert_eq!(
        rustix::io::Error::INVAL,
        rustix::io::Error::from_raw_os_error(rustix::io::Error::INVAL.raw_os_error())
    );
    #[cfg(not(windows))]
    assert_eq!(rustix::io::Error::INVAL.raw_os_error(), libc::EINVAL);
    #[cfg(windows)]
    assert_eq!(
        rustix::io::Error::INVAL.raw_os_error(),
        winapi::um::winsock2::WSAEINVAL
    );
}
