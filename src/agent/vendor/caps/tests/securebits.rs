use caps::securebits;

#[test]
fn test_keepcaps() {
    // Test a roundtrip on SET_KEEPCAPS.
    let f0 = securebits::has_keepcaps().unwrap();
    securebits::set_keepcaps(!f0).unwrap();
    let f1 = securebits::has_keepcaps().unwrap();
    assert_eq!(f0, !f1);
    securebits::set_keepcaps(!f1).unwrap();
    let f2 = securebits::has_keepcaps().unwrap();
    assert_eq!(f0, f2);
}
