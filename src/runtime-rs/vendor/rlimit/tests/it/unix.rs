use std::io::ErrorKind;

use rlimit::{getrlimit, setrlimit, Resource};

use super::{expect_err, expect_ok};

#[test]
fn resource_set_get() {
    const SOFT: u64 = 4 * 1024 * 1024;
    const HARD: u64 = 8 * 1024 * 1024;

    expect_ok(Resource::FSIZE.set(SOFT - 1, HARD));

    expect_ok(setrlimit(Resource::FSIZE, SOFT, HARD));

    assert_eq!(Resource::FSIZE.get().unwrap(), (SOFT, HARD));

    // FIXME: why does this line succeed on freebsd?
    #[cfg(not(target_os = "freebsd"))]
    {
        expect_err(Resource::FSIZE.set(HARD, SOFT), ErrorKind::InvalidInput);
    }

    expect_err(
        Resource::FSIZE.set(HARD, HARD + 1),
        ErrorKind::PermissionDenied,
    );
}

#[test]
fn resource_infinity() {
    assert_eq!(
        getrlimit(Resource::CPU).unwrap(),
        (rlimit::INFINITY, rlimit::INFINITY)
    );
}
