use hex_literal::hex;

use sha1collisiondetection::{Sha1CD, Output};

#[test]
fn shattered_1() {
    let input = &include_bytes!("../test/shattered-1.pdf")[..];

    // No detection.
    let mut ctx = Sha1CD::configure().detect_collisions(false).build();
    ctx.update(input);
    let d = ctx.finalize_cd().unwrap();
    assert_eq!(&d[..], hex!("38762cf7f55934b34d179ae6a4c80cadccbb7f0a"));

    // No mitigation.
    let mut ctx = Sha1CD::configure().safe_hash(false).build();
    ctx.update(input);
    let mut d = Output::default();
    let r = ctx.finalize_into_dirty_cd(&mut d);
    assert!(r.is_err());
    assert_eq!(&d[..], hex!("38762cf7f55934b34d179ae6a4c80cadccbb7f0a"));

    // No mitigation, no optimization.
    let mut ctx = Sha1CD::configure().safe_hash(false).use_ubc(false).build();
    ctx.update(input);
    let mut d = Output::default();
    let r = ctx.finalize_into_dirty_cd(&mut d);
    assert!(r.is_err());
    assert_eq!(&d[..], hex!("38762cf7f55934b34d179ae6a4c80cadccbb7f0a"));

    // With mitigation.
    let mut ctx = Sha1CD::default();
    ctx.update(input);
    let mut d = Output::default();
    let _ = ctx.finalize_into_dirty_cd(&mut d);
    assert_eq!(&d[..], hex!("16e96b70000dd1e7c85b8368ee197754400e58ec"));
}

#[test]
fn shattered_2() {
    let input = &include_bytes!("../test/shattered-2.pdf")[..];

    // No detection.
    let mut ctx = Sha1CD::configure().detect_collisions(false).build();
    ctx.update(input);
    let d = ctx.finalize_cd().unwrap();
    assert_eq!(&d[..], hex!("38762cf7f55934b34d179ae6a4c80cadccbb7f0a"));

    // No mitigation.
    let mut ctx = Sha1CD::configure().safe_hash(false).build();
    ctx.update(input);
    let mut d = Output::default();
    let r = ctx.finalize_into_dirty_cd(&mut d);
    assert!(r.is_err());
    assert_eq!(&d[..], hex!("38762cf7f55934b34d179ae6a4c80cadccbb7f0a"));

    // No mitigation, no optimization.
    let mut ctx = Sha1CD::configure().safe_hash(false).use_ubc(false).build();
    ctx.update(input);
    let mut d = Output::default();
    let r = ctx.finalize_into_dirty_cd(&mut d);
    assert!(r.is_err());
    assert_eq!(&d[..], hex!("38762cf7f55934b34d179ae6a4c80cadccbb7f0a"));

    // With mitigation.
    let mut ctx = Sha1CD::default();
    ctx.update(input);
    let mut d = Output::default();
    let _ = ctx.finalize_into_dirty_cd(&mut d);
    assert_eq!(&d[..], hex!("e1761773e6a35916d99f891b77663e6405313587"));
}
