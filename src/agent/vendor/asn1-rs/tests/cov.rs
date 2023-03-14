//! Generic and coverage tests
use asn1_rs::*;
use std::io;

#[test]
fn new_embedded_pdv() {
    fn create_pdv(identification: PdvIdentification) -> EmbeddedPdv {
        let pdv = EmbeddedPdv {
            identification,
            data_value_descriptor: None,
            data_value: &[0x00, 0xff],
        };
        assert!(pdv.data_value_descriptor.is_none());
        assert_eq!(pdv.data_value.len(), 2);
        pdv
    }
    let identification = PdvIdentification::ContextNegotiation {
        presentation_context_id: Integer::from(42_u8),
        presentation_syntax: oid! { 1.2.3.4.5 },
    };
    let pdv1 = create_pdv(identification);
    let identification = PdvIdentification::Syntaxes {
        s_abstract: oid! { 1.2.3 },
        s_transfer: oid! { 1.2.3.4.5 },
    };
    let pdv2 = create_pdv(identification);
    assert!(pdv1 != pdv2);
    let identification = PdvIdentification::Syntaxes {
        s_abstract: oid! { 1.2.3 },
        s_transfer: oid! { 1.2.3.4.5 },
    };
    let pdv3 = create_pdv(identification);
    assert!(pdv3 == pdv2);
}

#[test]
fn methods_error() {
    let e = Error::invalid_value(Tag(0), "msg".to_string());
    assert_eq!(
        e,
        Error::InvalidValue {
            tag: Tag(0),
            msg: "msg".to_string(),
        }
    );
    //
    let e = Error::unexpected_tag(None, Tag(0));
    assert_eq!(
        e,
        Error::UnexpectedTag {
            expected: None,
            actual: Tag(0),
        }
    );
    //
    let e = Error::unexpected_class(None, Class::Application);
    assert_eq!(
        e,
        Error::UnexpectedClass {
            expected: None,
            actual: Class::Application
        }
    );
    //
    use nom::error::ParseError;
    let e = Error::from_error_kind(&[], nom::error::ErrorKind::Fail);
    let e = <asn1_rs::Error as ParseError<_>>::append(&[], nom::error::ErrorKind::Eof, e);
    let s = format!("{}", e);
    assert!(s.starts_with("nom error:"));
    //
    let e1 = Error::from(nom::Err::Error(Error::BerTypeError));
    let e2 = Error::from(nom::Err::Incomplete(nom::Needed::new(2)));
    assert!(e1 != e2);
    //
    let e = SerializeError::from(Error::BerTypeError);
    let s = format!("{}", e);
    assert!(s.starts_with("ASN.1 error:"));
    //
    let e = SerializeError::InvalidClass { class: 4 };
    let s = format!("{}", e);
    assert!(s.starts_with("Invalid Class"));
    //
    let e = SerializeError::from(io::Error::new(io::ErrorKind::Other, "msg"));
    let s = format!("{}", e);
    assert!(s.starts_with("I/O error:"));
}

#[test]
fn methods_tag() {
    let t = Tag::from(2);
    assert_eq!(t, Tag::Integer);
    //
    let err = t.invalid_value("test");
    if let Error::InvalidValue { tag, .. } = err {
        assert_eq!(tag, Tag::Integer);
    } else {
        unreachable!();
    }
}
