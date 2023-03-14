use rustix::io::DupFlags;

/// `dup2` uses POSIX `dup2` which silently does nothing if the file
/// descriptors are equal.
#[test]
fn test_dup2() {
    let (a, b) = rustix::io::pipe().unwrap();
    rustix::io::dup2(&a, &a).unwrap();
    rustix::io::dup2(&b, &b).unwrap();
}

/// `dup3` uses Linux `dup3` which fails with `INVAL` if the
/// file descriptors are equal.
#[test]
fn test_dup3() {
    let (a, b) = rustix::io::pipe().unwrap();
    assert_eq!(
        rustix::io::dup3(&a, &a, DupFlags::empty()),
        Err(rustix::io::Error::INVAL)
    );
    assert_eq!(
        rustix::io::dup3(&b, &b, DupFlags::empty()),
        Err(rustix::io::Error::INVAL)
    );
    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_os = "redox"
    )))] // Android 5.0 has dup3, but libc doesn't have bindings
    {
        assert_eq!(
            rustix::io::dup3(&a, &a, DupFlags::CLOEXEC),
            Err(rustix::io::Error::INVAL)
        );
        assert_eq!(
            rustix::io::dup3(&b, &b, DupFlags::CLOEXEC),
            Err(rustix::io::Error::INVAL)
        );
    }
}
