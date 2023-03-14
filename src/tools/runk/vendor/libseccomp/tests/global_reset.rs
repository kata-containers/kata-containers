use libseccomp::*;

#[test]
fn test_reset_global_state() {
    if check_version(ScmpVersion::from((2, 5, 1))).unwrap() {
        assert!(reset_global_state().is_ok());
    } else {
        assert!(reset_global_state().is_err());
    }
}
