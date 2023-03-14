use hex_literal::hex;

use sha1collisiondetection::{Sha1CD, Output};

#[test]
fn shambles_1() {
    let input = &include_bytes!("../test/sha-mbles-1.bin")[..];

    // No detection.
    let mut ctx = Sha1CD::configure().detect_collisions(false).build();
    ctx.update(input);
    let d = ctx.finalize_cd().unwrap();
    assert_eq!(&d[..], hex!("8ac60ba76f1999a1ab70223f225aefdc78d4ddc0"));

    // No mitigation.
    let mut ctx = Sha1CD::configure().safe_hash(false).build();
    ctx.update(input);
    let mut d = Output::default();
    let r = ctx.finalize_into_dirty_cd(&mut d);
    assert!(r.is_err());
    assert_eq!(&d[..], hex!("8ac60ba76f1999a1ab70223f225aefdc78d4ddc0"));

    // No mitigation, no optimization.
    let mut ctx = Sha1CD::configure().safe_hash(false).use_ubc(false).build();
    ctx.update(input);
    let mut d = Output::default();
    let r = ctx.finalize_into_dirty_cd(&mut d);
    assert!(r.is_err());
    assert_eq!(&d[..], hex!("8ac60ba76f1999a1ab70223f225aefdc78d4ddc0"));

    // With mitigation.
    let mut ctx = Sha1CD::default();
    ctx.update(input);
    let mut d = Output::default();
    let _ = ctx.finalize_into_dirty_cd(&mut d);
    assert_eq!(&d[..], hex!("4f3d9be4a472c4dae83c6314aa6c36a064c1fd14"));
}

#[test]
fn shambles_2() {
    let input = &include_bytes!("../test/sha-mbles-2.bin")[..];

    // No detection.
    let mut ctx = Sha1CD::configure().detect_collisions(false).build();
    ctx.update(input);
    let d = ctx.finalize_cd().unwrap();
    assert_eq!(&d[..], hex!("8ac60ba76f1999a1ab70223f225aefdc78d4ddc0"));

    // No mitigation.
    let mut ctx = Sha1CD::configure().safe_hash(false).build();
    ctx.update(input);
    let mut d = Output::default();
    let r = ctx.finalize_into_dirty_cd(&mut d);
    assert!(r.is_err());
    assert_eq!(&d[..], hex!("8ac60ba76f1999a1ab70223f225aefdc78d4ddc0"));

    // No mitigation, no optimization.
    let mut ctx = Sha1CD::configure().safe_hash(false).use_ubc(false).build();
    ctx.update(input);
    let mut d = Output::default();
    let r = ctx.finalize_into_dirty_cd(&mut d);
    assert!(r.is_err());
    assert_eq!(&d[..], hex!("8ac60ba76f1999a1ab70223f225aefdc78d4ddc0"));

    // With mitigation.
    let mut ctx = Sha1CD::default();
    ctx.update(input);
    let mut d = Output::default();
    let _ = ctx.finalize_into_dirty_cd(&mut d);
    assert_eq!(&d[..], hex!("9ed5d77a4f48be1dbf3e9e15650733eb850897f2"));
}
