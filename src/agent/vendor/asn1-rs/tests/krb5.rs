//! Test implementation for Kerberos v5
//!
//! This is mostly used to verify that required types and functions are implemented,
//! and that provided API is convenient.

use asn1_rs::*;
use hex_literal::hex;

const PRINCIPAL_NAME: &[u8] = &hex!("30 81 11 a0 03 02 01 00 a1 0a 30 81 07 1b 05 4a 6f 6e 65 73");

/// PrincipalName   ::= SEQUENCE {
///         name-type       [0] Int32,
///         name-string     [1] SEQUENCE OF KerberosString
/// }
#[derive(Debug, PartialEq)]
pub struct PrincipalName {
    pub name_type: NameType,
    pub name_string: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NameType(pub i32);

// KerberosString  ::= GeneralString (IA5String)
pub type KerberosString<'a> = GeneralString<'a>;

pub type KerberosStringList<'a> = Vec<KerberosString<'a>>;

impl Tagged for PrincipalName {
    const TAG: Tag = Tag::Sequence;
}

impl<'a> FromDer<'a> for PrincipalName {
    fn from_der(bytes: &'a [u8]) -> ParseResult<'a, Self> {
        // XXX in the example above, PRINCIPAL_NAME does not respect DER constraints (length is using long form while < 127)
        let (rem, seq) = Sequence::from_ber(bytes)?;
        seq.and_then(|data| {
            let input = &data;
            let (i, t) = parse_der_tagged_explicit::<_, u32, _>(0)(input)?;
            let name_type = t.inner;
            let name_type = NameType(name_type as i32);
            let (_, t) = parse_der_tagged_explicit::<_, KerberosStringList, _>(1)(i)?;
            let name_string = t.inner.iter().map(|s| s.string()).collect();
            Ok((
                rem,
                PrincipalName {
                    name_type,
                    name_string,
                },
            ))
        })
    }
}

impl ToDer for PrincipalName {
    fn to_der_len(&self) -> Result<usize> {
        let sz = self.name_type.0.to_der_len()? + 2 /* tagged */;
        let sz = sz + self.name_string.to_der_len()? + 2 /* tagged */;
        Ok(sz)
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let len = self.to_der_len()?;
        let header = Header::new(Class::Universal, true, Self::TAG, Length::Definite(len));
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        // build DER sequence content
        let sz1 = self
            .name_type
            .0
            .explicit(Class::ContextSpecific, 0)
            .write_der(writer)?;
        let sz2 = self
            .name_string
            .iter()
            .map(|s| KerberosString::from(s.as_ref()))
            .collect::<Vec<_>>()
            .explicit(Class::ContextSpecific, 1)
            .write_der(writer)?;
        Ok(sz1 + sz2)
    }
}

#[test]
fn krb5_principalname() {
    let input = PRINCIPAL_NAME;
    let (rem, res) = PrincipalName::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    let expected = PrincipalName {
        name_type: NameType(0),
        name_string: vec!["Jones".to_string()],
    };
    assert_eq!(res, expected);
}

#[test]
fn to_der_krb5_principalname() {
    let principal = PrincipalName {
        name_type: NameType(0),
        name_string: vec!["Jones".to_string()],
    };
    let v = PrincipalName::to_der_vec(&principal).expect("serialization failed");
    std::fs::write("/tmp/out.bin", &v).unwrap();
    let (_, principal2) = PrincipalName::from_der(&v).expect("parsing failed");
    assert!(principal.eq(&principal2));
}
