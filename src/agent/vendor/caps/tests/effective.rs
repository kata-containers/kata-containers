#[test]
fn test_effective_has_cap() {
    caps::has_cap(None, caps::CapSet::Effective, caps::Capability::CAP_CHOWN).unwrap();
}

#[test]
fn test_effective_read() {
    caps::read(None, caps::CapSet::Effective).unwrap();
}

#[test]
fn test_effective_clear() {
    caps::clear(None, caps::CapSet::Effective).unwrap();
    let empty = caps::read(None, caps::CapSet::Effective).unwrap();
    assert_eq!(empty.len(), 0);
}

#[test]
fn test_effective_drop() {
    caps::drop(None, caps::CapSet::Effective, caps::Capability::CAP_CHOWN).unwrap();
    let no_eff = caps::has_cap(None, caps::CapSet::Effective, caps::Capability::CAP_CHOWN).unwrap();
    assert_eq!(no_eff, false);
}

#[test]
fn test_effective_raise() {
    let perm = caps::has_cap(None, caps::CapSet::Permitted, caps::Capability::CAP_CHOWN).unwrap();
    caps::drop(None, caps::CapSet::Effective, caps::Capability::CAP_CHOWN).unwrap();
    let r = caps::raise(None, caps::CapSet::Effective, caps::Capability::CAP_CHOWN);
    if perm {
        r.unwrap();
    } else {
        assert!(r.is_err());
    }
}

#[test]
fn test_effective_set() {
    let mut v = caps::CapsHashSet::new();
    caps::set(None, caps::CapSet::Effective, &v).unwrap();
    let empty = caps::read(None, caps::CapSet::Effective).unwrap();
    assert_eq!(empty.len(), 0);
    v.insert(caps::Capability::CAP_CHOWN);
    caps::drop(None, caps::CapSet::Ambient, caps::Capability::CAP_CHOWN).unwrap();
    assert!(caps::set(None, caps::CapSet::Ambient, &v).is_err());
}
