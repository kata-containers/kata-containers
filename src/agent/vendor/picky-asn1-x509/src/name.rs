use crate::{AttributeTypeAndValue, AttributeTypeAndValueParameters, DirectoryString};
use picky_asn1::tag::{Encoding, Tag, TagClass, TagPeeker};
use picky_asn1::wrapper::*;
use serde::{de, ser, Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum NameAttr {
    CommonName,
    Surname,
    SerialNumber,
    CountryName,
    LocalityName,
    StateOrProvinceName,
    StreetName,
    OrganizationName,
    OrganizationalUnitName,
}

/// [RFC 5280 #4.1.2.4](https://tools.ietf.org/html/rfc5280#section-4.1.2.4)
///
/// ```not_rust
/// RDNSequence ::= SEQUENCE OF RelativeDistinguishedName
/// ```
pub type RdnSequence = Asn1SequenceOf<RelativeDistinguishedName>;

/// [RFC 5280 #4.1.2.4](https://tools.ietf.org/html/rfc5280#section-4.1.2.4)
///
/// ```not_rust
/// RelativeDistinguishedName ::= SET SIZE (1..MAX) OF AttributeTypeAndValue
/// ```
pub type RelativeDistinguishedName = Asn1SetOf<AttributeTypeAndValue>;

/// [RFC 5280 #4.2.1.6](https://tools.ietf.org/html/rfc5280#section-4.2.1.6)
///
/// ```not_rust
/// DirectoryName ::= Name
/// ```
pub type DirectoryName = Name;

/// [RFC 5280 #4.1.2.4](https://tools.ietf.org/html/rfc5280#section-4.1.2.4)
///
/// ```not_rust
/// Name ::= CHOICE { -- only one possibility for now --
///       rdnSequence  RDNSequence }
/// ```
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Name(pub RdnSequence);

impl Default for Name {
    fn default() -> Self {
        Self::new()
    }
}

impl Name {
    pub fn new() -> Self {
        Self(Asn1SequenceOf(Vec::new()))
    }

    pub fn new_common_name<S: Into<DirectoryString>>(name: S) -> Self {
        let mut dn = Self::default();
        dn.add_attr(NameAttr::CommonName, name);
        dn
    }

    /// Find the first common name contained in this `Name`
    pub fn find_common_name(&self) -> Option<&DirectoryString> {
        for relative_distinguished_name in &((self.0).0) {
            for attr_ty_val in &relative_distinguished_name.0 {
                if let AttributeTypeAndValueParameters::CommonName(dir_string) = &attr_ty_val.value {
                    return Some(dir_string);
                }
            }
        }
        None
    }

    pub fn add_attr<S: Into<DirectoryString>>(&mut self, attr: NameAttr, value: S) {
        let ty_val = match attr {
            NameAttr::CommonName => AttributeTypeAndValue::new_common_name(value),
            NameAttr::Surname => AttributeTypeAndValue::new_surname(value),
            NameAttr::SerialNumber => AttributeTypeAndValue::new_serial_number(value),
            NameAttr::CountryName => AttributeTypeAndValue::new_country_name(value),
            NameAttr::LocalityName => AttributeTypeAndValue::new_locality_name(value),
            NameAttr::StateOrProvinceName => AttributeTypeAndValue::new_state_or_province_name(value),
            NameAttr::StreetName => AttributeTypeAndValue::new_street_name(value),
            NameAttr::OrganizationName => AttributeTypeAndValue::new_organization_name(value),
            NameAttr::OrganizationalUnitName => AttributeTypeAndValue::new_organizational_unit_name(value),
        };
        let set_val = Asn1SetOf(vec![ty_val]);
        ((self.0).0).push(set_val);
    }

    /// Add an emailAddress attribute.
    /// NOTE: this attribute does not conform with the RFC 5280, email should be placed in SAN instead
    pub fn add_email<S: Into<IA5StringAsn1>>(&mut self, value: S) {
        let set_val = Asn1SetOf(vec![AttributeTypeAndValue::new_email_address(value)]);
        ((self.0).0).push(set_val);
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        NamePrettyFormatter(self).fmt(f)
    }
}

pub struct NamePrettyFormatter<'a>(pub &'a Name);

impl fmt::Display for NamePrettyFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for name in &((self.0).0).0 {
            for attr in &name.0 {
                if first {
                    first = false;
                } else {
                    write!(f, ",")?;
                }

                match &attr.value {
                    AttributeTypeAndValueParameters::CommonName(name) => {
                        write!(f, "CN={}", name)?;
                    }
                    AttributeTypeAndValueParameters::Surname(name) => {
                        write!(f, "SURNAME={}", name)?;
                    }
                    AttributeTypeAndValueParameters::SerialNumber(name) => {
                        write!(f, "SN={}", name)?;
                    }
                    AttributeTypeAndValueParameters::CountryName(name) => {
                        write!(f, "C={}", name)?;
                    }
                    AttributeTypeAndValueParameters::LocalityName(name) => {
                        write!(f, "L={}", name)?;
                    }
                    AttributeTypeAndValueParameters::StateOrProvinceName(name) => {
                        write!(f, "ST={}", name)?;
                    }
                    AttributeTypeAndValueParameters::StreetName(name) => {
                        write!(f, "STREET NAME={}", name)?;
                    }
                    AttributeTypeAndValueParameters::OrganizationName(name) => {
                        write!(f, "O={}", name)?;
                    }
                    AttributeTypeAndValueParameters::OrganizationalUnitName(name) => {
                        write!(f, "OU={}", name)?;
                    }
                    AttributeTypeAndValueParameters::EmailAddress(name) => {
                        write!(f, "EMAIL={}", String::from_utf8_lossy(name.as_bytes()))?;
                    }
                    AttributeTypeAndValueParameters::Custom(der) => {
                        write!(f, "{}={:?}", Into::<String>::into(&attr.ty.0), der)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// [RFC 5280 #4.2.1.6](https://tools.ietf.org/html/rfc5280#section-4.2.1.6)
///
/// ```not_rust
/// GeneralNames ::= SEQUENCE SIZE (1..MAX) OF GeneralName
/// ```
pub type GeneralNames = Asn1SequenceOf<GeneralName>;

/// [RFC 5280 #4.2.1.6](https://tools.ietf.org/html/rfc5280#section-4.2.1.6)
///
/// ```not_rust
/// GeneralName ::= CHOICE {
///       otherName                       [0]     OtherName,
///       rfc822Name                      [1]     IA5String,
///       dNSName                         [2]     IA5String,
///       x400Address                     [3]     ORAddress,
///       directoryName                   [4]     Name,
///       ediPartyName                    [5]     EDIPartyName,
///       uniformResourceIdentifier       [6]     IA5String,
///       iPAddress                       [7]     OCTET STRING,
///       registeredID                    [8]     OBJECT IDENTIFIER }
/// ```
#[derive(Debug, PartialEq, Clone)]
pub enum GeneralName {
    OtherName(OtherName),
    Rfc822Name(IA5StringAsn1),
    DnsName(IA5StringAsn1),
    //X400Address(ORAddress),
    DirectoryName(Name),
    EdiPartyName(EdiPartyName),
    Uri(IA5StringAsn1),
    IpAddress(OctetStringAsn1),
    RegisteredId(ObjectIdentifierAsn1),
}

impl GeneralName {
    pub fn new_edi_party_name<PN, NA>(party_name: PN, name_assigner: Option<NA>) -> Self
    where
        PN: Into<DirectoryString>,
        NA: Into<DirectoryString>,
    {
        Self::EdiPartyName(EdiPartyName {
            name_assigner: Optional(name_assigner.map(Into::into).map(ImplicitContextTag0)),
            party_name: ImplicitContextTag1(party_name.into()),
        })
    }
}

impl From<Name> for GeneralName {
    fn from(name: Name) -> Self {
        Self::DirectoryName(name)
    }
}

impl ser::Serialize for GeneralName {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            GeneralName::OtherName(name) => {
                let mut raw_der = picky_asn1_der::to_vec(name).map_err(ser::Error::custom)?;
                raw_der[0] = Tag::context_specific_constructed(0).inner();
                picky_asn1_der::Asn1RawDer(raw_der).serialize(serializer)
            }
            GeneralName::Rfc822Name(name) => ImplicitContextTag1(name).serialize(serializer),
            GeneralName::DnsName(name) => ImplicitContextTag2(name).serialize(serializer),
            GeneralName::DirectoryName(name) => ImplicitContextTag4(name).serialize(serializer),
            GeneralName::EdiPartyName(name) => ImplicitContextTag5(name).serialize(serializer),
            GeneralName::Uri(name) => ImplicitContextTag6(name).serialize(serializer),
            GeneralName::IpAddress(name) => ImplicitContextTag7(name).serialize(serializer),
            GeneralName::RegisteredId(name) => ImplicitContextTag8(name).serialize(serializer),
        }
    }
}

impl<'de> de::Deserialize<'de> for GeneralName {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = GeneralName;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded GeneralName")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let tag_peeker: TagPeeker = seq_next_element!(seq, DirectoryString, "choice tag");
                match tag_peeker.next_tag.components() {
                    (TagClass::ContextSpecific, Encoding::Primitive, 0) => Err(serde_invalid_value!(
                        GeneralName,
                        "Primitive encoding for OtherName not supported",
                        "a supported choice"
                    )),
                    (TagClass::ContextSpecific, Encoding::Constructed, 0) => Ok(GeneralName::OtherName(
                        seq_next_element!(seq, OtherName, GeneralName, "OtherName"),
                    )),
                    (TagClass::ContextSpecific, Encoding::Primitive, 1) => Ok(GeneralName::Rfc822Name(
                        seq_next_element!(seq, ImplicitContextTag1<IA5StringAsn1>, GeneralName, "RFC822Name").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Constructed, 1) => Ok(GeneralName::Rfc822Name(
                        seq_next_element!(seq, ExplicitContextTag1<IA5StringAsn1>, GeneralName, "RFC822Name").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Primitive, 2) => Ok(GeneralName::DnsName(
                        seq_next_element!(seq, ImplicitContextTag2<IA5StringAsn1>, GeneralName, "DNSName").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Constructed, 2) => Ok(GeneralName::DnsName(
                        seq_next_element!(seq, ExplicitContextTag2<IA5StringAsn1>, GeneralName, "DNSName").0,
                    )),
                    (TagClass::ContextSpecific, _, 3) => Err(serde_invalid_value!(
                        GeneralName,
                        "X400Address not supported",
                        "a supported choice"
                    )),
                    (TagClass::ContextSpecific, Encoding::Primitive, 4) => Ok(GeneralName::DirectoryName(
                        seq_next_element!(seq, ImplicitContextTag4<Name>, GeneralName, "DirectoryName").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Constructed, 4) => Ok(GeneralName::DirectoryName(
                        seq_next_element!(seq, ExplicitContextTag4<Name>, GeneralName, "DirectoryName").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Primitive, 5) => Ok(GeneralName::EdiPartyName(
                        seq_next_element!(seq, ImplicitContextTag5<EdiPartyName>, GeneralName, "EDIPartyName").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Constructed, 5) => Ok(GeneralName::EdiPartyName(
                        seq_next_element!(seq, ExplicitContextTag5<EdiPartyName>, GeneralName, "EDIPartyName").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Primitive, 6) => Ok(GeneralName::Uri(
                        seq_next_element!(seq, ImplicitContextTag6<IA5StringAsn1>, GeneralName, "URI").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Constructed, 6) => Ok(GeneralName::Uri(
                        seq_next_element!(seq, ExplicitContextTag6<IA5StringAsn1>, GeneralName, "URI").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Primitive, 7) => Ok(GeneralName::IpAddress(
                        seq_next_element!(seq, ImplicitContextTag7<OctetStringAsn1>, GeneralName, "IpAddress").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Constructed, 7) => Ok(GeneralName::IpAddress(
                        seq_next_element!(seq, ExplicitContextTag7<OctetStringAsn1>, GeneralName, "IpAddress").0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Primitive, 8) => Ok(GeneralName::RegisteredId(
                        seq_next_element!(
                            seq,
                            ImplicitContextTag8<ObjectIdentifierAsn1>,
                            GeneralName,
                            "RegisteredId"
                        )
                        .0,
                    )),
                    (TagClass::ContextSpecific, Encoding::Constructed, 8) => Ok(GeneralName::RegisteredId(
                        seq_next_element!(
                            seq,
                            ExplicitContextTag8<ObjectIdentifierAsn1>,
                            GeneralName,
                            "RegisteredId"
                        )
                        .0,
                    )),
                    _ => Err(serde_invalid_value!(
                        GeneralName,
                        "unknown choice value",
                        "a supported GeneralName choice"
                    )),
                }
            }
        }

        deserializer.deserialize_enum(
            "GeneralName",
            &[
                "RFC822Name",
                "DNSName",
                "DirectoryName",
                "EDIPartyName",
                "URI",
                "IpAddress",
                "RegisteredId",
            ],
            Visitor,
        )
    }
}

// OtherName ::= SEQUENCE {
//      type-id    OBJECT IDENTIFIER,
//      value      [0] EXPLICIT ANY DEFINED BY type-id}
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct OtherName {
    pub type_id: ObjectIdentifierAsn1,
    pub value: ExplicitContextTag0<picky_asn1_der::Asn1RawDer>,
}

/// [RFC 5280 #4.2.1.6](https://tools.ietf.org/html/rfc5280#section-4.2.1.6)
///
/// ```not_rust
/// EDIPartyName ::= SEQUENCE {
///      nameAssigner            [0]     DirectoryString OPTIONAL,
///      partyName               [1]     DirectoryString }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct EdiPartyName {
    pub name_assigner: Optional<Option<ImplicitContextTag0<DirectoryString>>>,
    pub party_name: ImplicitContextTag1<DirectoryString>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use oid::ObjectIdentifier;
    use picky_asn1::restricted_string::IA5String;
    use picky_asn1_der::Asn1RawDer;
    use std::str::FromStr;

    #[test]
    fn common_name() {
        #[rustfmt::skip]
        let encoded = [
            0x30, 0x1D, // sequence
            0x31, 0x1B, // set
            0x30, 0x19, // sequence
            0x06, 0x03, // tag of oid
            0x55, 0x04, 0x03, // oid of common name
            0x0c, 0x12,  // tag of utf-8 string
            0x74, 0x65, 0x73, 0x74, 0x2E, 0x63, 0x6F, 0x6E, 0x74, 0x6F,
            0x73, 0x6F, 0x2E, 0x6C, 0x6F, 0x63, 0x61, 0x6C, // utf8 string
        ];
        let expected = Name::new_common_name("test.contoso.local");
        check_serde!(expected: Name in encoded);
    }

    #[test]
    fn multiple_attributes() {
        #[rustfmt::skip]
        let encoded = [
            0x30, 0x52, // sequence, 0x52(82) bytes
            0x31, 0x1B, // set 1 (common name), 0x1b(27) bytes
            0x30, 0x19, // sequence, 0x19(25) bytes
            0x06, 0x03, // oid tag
            0x55, 0x04, 0x03, // oid of common name attribute
            0x0c, 0x12,  // tag of utf-8 string
            b't', b'e', b's', b't', b'.', b'c', b'o', b'n', b't', b'o', b's', b'o', b'.', b'l', b'o', b'c', b'a', b'l',

            0x31, 0x10, // set 2 (locality)
            0x30, 0x0E, // sequence
            0x06, 0x03, //oid tag
            0x55, 0x04, 0x07, // oid of locality attribute
            0x0c, 0x07,  // tag of utf-8 string
            b'U', b'n', b'k', b'n', b'o', b'w', b'n', // utf8 string data

            0x31, 0x21, // set 3 (emailAddress)
            0x30, 0x1F, // sequence
            0x06, 0x09, // oid tag
            0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x01, // oid of emailAddress
            0x16, 0x12,  // tag of IA5String
            b's', b'o', b'm', b'e', b'@', b'c', b'o', b'n', b't', b'o', b's', b'o', b'.', b'l', b'o', b'c', b'a', b'l', // utf-8 string data
            ];
        let mut expected = Name::new_common_name("test.contoso.local");
        expected.add_attr(NameAttr::LocalityName, "Unknown");
        let email = IA5StringAsn1(IA5String::from_str("some@contoso.local").unwrap());
        expected.add_email(email);
        check_serde!(expected: Name in encoded);
    }

    #[test]
    fn general_name_dns() {
        #[rustfmt::skip]
        let encoded = [
            0x82, 0x11,
            0x64, 0x65, 0x76, 0x65, 0x6C, 0x2E, 0x65, 0x78, 0x61, 0x6D, 0x70, 0x6C, 0x65, 0x2E, 0x63, 0x6F, 0x6D,
        ];
        let expected = GeneralName::DnsName(IA5String::from_string("devel.example.com".into()).unwrap().into());
        check_serde!(expected: GeneralName in encoded);
    }

    #[test]
    fn general_name_other_name() {
        let encoded = [
            160, 24, 6, 8, 43, 6, 1, 5, 5, 7, 8, 3, 160, 12, 48, 10, 12, 8, 65, 69, 45, 57, 52, 51, 52, 57,
        ];

        let expected = GeneralName::OtherName(OtherName {
            type_id: ObjectIdentifierAsn1(ObjectIdentifier::try_from("1.3.6.1.5.5.7.8.3").unwrap()),
            value: ExplicitContextTag0::from(Asn1RawDer(vec![48, 10, 12, 8, 65, 69, 45, 57, 52, 51, 52, 57])),
        });

        check_serde!(expected: GeneralName in encoded);
    }
}
