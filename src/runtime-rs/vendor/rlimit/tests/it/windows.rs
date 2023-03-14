use super::{expect_err, expect_ok};

use std::io;

#[test]
fn maxstdio() {
    assert_eq!(rlimit::getmaxstdio(), 512);
    assert_eq!(rlimit::setmaxstdio(2048).unwrap(), 2048);
    assert_eq!(rlimit::getmaxstdio(), 2048);
}
