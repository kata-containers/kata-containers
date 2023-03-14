// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::*;

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_return() {
    let f = || {
        fail::fail_point!("return", |s: Option<String>| s
            .map_or(2, |s| s.parse().unwrap()));
        0
    };
    assert_eq!(f(), 0);

    fail::cfg("return", "return(1000)").unwrap();
    assert_eq!(f(), 1000);

    fail::cfg("return", "return").unwrap();
    assert_eq!(f(), 2);
}
