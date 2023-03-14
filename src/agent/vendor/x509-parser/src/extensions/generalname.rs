use crate::error::{X509Error, X509Result};
use crate::prelude::format_serial;
use crate::x509::X509Name;
use asn1_rs::{Any, CheckDerConstraints, Class, Error, FromDer, Oid, Sequence};
use core::convert::TryFrom;
use nom::combinator::all_consuming;
use nom::{Err, IResult};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
/// Represents a GeneralName as defined in RFC5280. There
/// is no support X.400 addresses and EDIPartyName.
///
/// String formats are not validated.
pub enum GeneralName<'a> {
    OtherName(Oid<'a>, &'a [u8]),
    /// More or less an e-mail, the format is not checked.
    RFC822Name(&'a str),
    /// A hostname, format is not checked.
    DNSName(&'a str),
    /// X400Address,
    X400Address(Any<'a>),
    /// RFC5280 defines several string types, we always try to parse as utf-8
    /// which is more or less a superset of the string types.
    DirectoryName(X509Name<'a>),
    /// EDIPartyName
    EDIPartyName(Any<'a>),
    /// An uniform resource identifier. The format is not checked.
    URI(&'a str),
    /// An ip address, provided as encoded.
    IPAddress(&'a [u8]),
    RegisteredID(Oid<'a>),
}

impl<'a> TryFrom<Any<'a>> for GeneralName<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self, Self::Error> {
        any.class().assert_eq(Class::ContextSpecific)?;
        fn ia5str(any: Any) -> Result<&str, Err<Error>> {
            // Relax constraints from RFC here: we are expecting an IA5String, but many certificates
            // are using unicode characters
            std::str::from_utf8(any.data).map_err(|_| nom::Err::Failure(Error::BerValueError))
        }
        let name = match any.tag().0 {
            0 => {
                // otherName SEQUENCE { OID, [0] explicit any defined by oid }
                let (rest, oid) = Oid::from_der(any.data)?;
                GeneralName::OtherName(oid, rest)
            }
            1 => GeneralName::RFC822Name(ia5str(any)?),
            2 => GeneralName::DNSName(ia5str(any)?),
            3 => {
                // XXX Not yet implemented
                GeneralName::X400Address(any)
            }
            4 => {
                // directoryName, name
                let (_, name) = all_consuming(X509Name::from_der)(any.data)
                    .or(Err(Error::Unsupported)) // XXX remove me
                    ?;
                GeneralName::DirectoryName(name)
            }
            5 => {
                // XXX Not yet implemented
                GeneralName::EDIPartyName(any)
            }
            6 => GeneralName::URI(ia5str(any)?),
            7 => {
                // IPAddress, OctetString
                GeneralName::IPAddress(any.data)
            }
            8 => {
                let oid = Oid::new(any.data.into());
                GeneralName::RegisteredID(oid)
            }
            _ => return Err(Error::unexpected_tag(None, any.tag())),
        };
        Ok(name)
    }
}

impl CheckDerConstraints for GeneralName<'_> {
    fn check_constraints(any: &Any) -> asn1_rs::Result<()> {
        Sequence::check_constraints(any)
    }
}

impl<'a> FromDer<'a, X509Error> for GeneralName<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_generalname(i).map_err(Err::convert)
    }
}

impl<'a> fmt::Display for GeneralName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeneralName::OtherName(oid, _) => write!(f, "OtherName({}, [...])", oid),
            GeneralName::RFC822Name(s) => write!(f, "RFC822Name({})", s),
            GeneralName::DNSName(s) => write!(f, "DNSName({})", s),
            GeneralName::X400Address(_) => write!(f, "X400Address(<unparsed>)"),
            GeneralName::DirectoryName(dn) => write!(f, "DirectoryName({})", dn),
            GeneralName::EDIPartyName(_) => write!(f, "EDIPartyName(<unparsed>)"),
            GeneralName::URI(s) => write!(f, "URI({})", s),
            GeneralName::IPAddress(b) => write!(f, "IPAddress({})", format_serial(b)),
            GeneralName::RegisteredID(oid) => write!(f, "RegisteredID({})", oid),
        }
    }
}

pub(crate) fn parse_generalname<'a>(i: &'a [u8]) -> IResult<&'a [u8], GeneralName, Error> {
    let (rest, any) = Any::from_der(i)?;
    let gn = GeneralName::try_from(any)?;
    Ok((rest, gn))
}
