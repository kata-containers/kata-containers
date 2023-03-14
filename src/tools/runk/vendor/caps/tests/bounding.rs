#[test]
fn test_bounding_has_cap() {
    caps::has_cap(
        None,
        caps::CapSet::Bounding,
        caps::Capability::CAP_SYS_CHROOT,
    )
    .unwrap();
}

#[test]
fn test_bounding_read() {
    caps::read(None, caps::CapSet::Bounding).unwrap();
}

#[test]
fn test_bounding_clear() {
    let ret = caps::clear(None, caps::CapSet::Bounding);
    if caps::has_cap(None, caps::CapSet::Effective, caps::Capability::CAP_SETPCAP).unwrap() {
        ret.unwrap();
        let empty = caps::read(None, caps::CapSet::Bounding).unwrap();
        assert_eq!(empty.len(), 0);
    } else {
        assert!(ret.is_err());
    };
}

#[test]
fn test_bounding_drop() {
    let ret = caps::drop(
        None,
        caps::CapSet::Bounding,
        caps::Capability::CAP_SYS_CHROOT,
    );
    if caps::has_cap(None, caps::CapSet::Effective, caps::Capability::CAP_SETPCAP).unwrap() {
        ret.unwrap();
        let set = caps::read(None, caps::CapSet::Bounding).unwrap();
        assert!(!set.contains(&caps::Capability::CAP_SYS_CHROOT));
    } else {
        assert!(ret.is_err());
    }
}

#[test]
fn test_bounding_drop_other() {
    assert!(caps::drop(Some(1), caps::CapSet::Bounding, caps::Capability::CAP_CHOWN).is_err());
}

#[test]
fn test_bounding_raise() {
    assert!(caps::raise(None, caps::CapSet::Bounding, caps::Capability::CAP_CHOWN).is_err());
}

#[test]
fn test_bounding_set() {
    let v = caps::CapsHashSet::new();
    assert!(caps::set(None, caps::CapSet::Bounding, &v).is_err());
}
