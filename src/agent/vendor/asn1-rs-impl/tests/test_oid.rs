use asn1_rs_impl::{encode_int, encode_oid};

#[test]
fn test_encode_oid() {
    // example from http://luca.ntop.org/Teaching/Appunti/asn1.html
    let oid = encode_oid! {1.2.840.113549};
    assert_eq!(oid, [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d]);
}

#[test]
fn test_encode_int() {
    //
    let int = encode_int!(1234);
    assert_eq!(int, [0x04, 0xd2]);
}
