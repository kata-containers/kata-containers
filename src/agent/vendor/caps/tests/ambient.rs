#[test]
fn test_ambient_has_cap() {
    caps::has_cap(None, caps::CapSet::Ambient, caps::Capability::CAP_CHOWN).unwrap();
}

#[test]
fn test_ambient_read() {
    caps::read(None, caps::CapSet::Ambient).unwrap();
}

#[test]
fn test_ambient_clear() {
    caps::clear(None, caps::CapSet::Ambient).unwrap();
    let empty = caps::read(None, caps::CapSet::Ambient).unwrap();
    assert_eq!(empty.len(), 0);
}

#[test]
fn test_ambient_drop() {
    caps::drop(None, caps::CapSet::Ambient, caps::Capability::CAP_CHOWN).unwrap();
    let no_cap = caps::has_cap(None, caps::CapSet::Ambient, caps::Capability::CAP_CHOWN).unwrap();
    assert_eq!(no_cap, false);
}

#[test]
fn test_ambient_drop_other() {
    assert!(caps::drop(Some(1), caps::CapSet::Ambient, caps::Capability::CAP_CHOWN).is_err());
}

#[test]
fn test_ambient_raise() {
    let r = caps::raise(None, caps::CapSet::Ambient, caps::Capability::CAP_CHOWN);
    let perm = caps::has_cap(None, caps::CapSet::Permitted, caps::Capability::CAP_CHOWN).unwrap();
    let inhe = caps::has_cap(None, caps::CapSet::Inheritable, caps::Capability::CAP_CHOWN).unwrap();
    match (perm, inhe) {
        (false, _) => assert!(r.is_err()),
        (true, false) => {
            caps::raise(None, caps::CapSet::Inheritable, caps::Capability::CAP_CHOWN).unwrap();
            caps::raise(None, caps::CapSet::Ambient, caps::Capability::CAP_CHOWN).unwrap();
        }
        (true, true) => r.unwrap(),
    };
}

#[test]
fn test_ambient_set() {
    let mut v = caps::CapsHashSet::new();
    caps::set(None, caps::CapSet::Ambient, &v).unwrap();
    let empty = caps::read(None, caps::CapSet::Ambient).unwrap();
    assert_eq!(empty.len(), 0);
    v.insert(caps::Capability::CAP_CHOWN);
    caps::drop(None, caps::CapSet::Ambient, caps::Capability::CAP_CHOWN).unwrap();
    assert!(caps::set(None, caps::CapSet::Ambient, &v).is_err());
}
