use crate::{oids, DirectoryString};
use picky_asn1::wrapper::{IA5StringAsn1, ObjectIdentifierAsn1};
use serde::{de, ser};
use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub enum AttributeTypeAndValueParameters {
    CommonName(DirectoryString),
    Surname(DirectoryString),
    SerialNumber(DirectoryString),
    CountryName(DirectoryString),
    LocalityName(DirectoryString),
    StateOrProvinceName(DirectoryString),
    StreetName(DirectoryString),
    OrganizationName(DirectoryString),
    OrganizationalUnitName(DirectoryString),
    EmailAddress(IA5StringAsn1),
    Custom(picky_asn1_der::Asn1RawDer),
}

#[derive(Debug, PartialEq, Clone)]
pub struct AttributeTypeAndValue {
    pub ty: ObjectIdentifierAsn1,
    pub value: AttributeTypeAndValueParameters,
}

impl AttributeTypeAndValue {
    pub fn new_common_name<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_common_name().into(),
            value: AttributeTypeAndValueParameters::CommonName(name.into()),
        }
    }

    pub fn new_surname<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_surname().into(),
            value: AttributeTypeAndValueParameters::Surname(name.into()),
        }
    }

    pub fn new_serial_number<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_serial_number().into(),
            value: AttributeTypeAndValueParameters::SerialNumber(name.into()),
        }
    }

    pub fn new_country_name<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_country_name().into(),
            value: AttributeTypeAndValueParameters::CountryName(name.into()),
        }
    }

    pub fn new_locality_name<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_locality_name().into(),
            value: AttributeTypeAndValueParameters::LocalityName(name.into()),
        }
    }

    pub fn new_state_or_province_name<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_state_or_province_name().into(),
            value: AttributeTypeAndValueParameters::StateOrProvinceName(name.into()),
        }
    }

    pub fn new_street_name<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_street_name().into(),
            value: AttributeTypeAndValueParameters::StreetName(name.into()),
        }
    }

    pub fn new_organization_name<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_organization_name().into(),
            value: AttributeTypeAndValueParameters::OrganizationName(name.into()),
        }
    }

    pub fn new_organizational_unit_name<S: Into<DirectoryString>>(name: S) -> Self {
        Self {
            ty: oids::at_organizational_unit_name().into(),
            value: AttributeTypeAndValueParameters::OrganizationalUnitName(name.into()),
        }
    }

    pub fn new_email_address<S: Into<IA5StringAsn1>>(name: S) -> Self {
        Self {
            ty: oids::email_address().into(),
            value: AttributeTypeAndValueParameters::EmailAddress(name.into()),
        }
    }
}

impl ser::Serialize for AttributeTypeAndValue {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        use ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.ty)?;
        match &self.value {
            AttributeTypeAndValueParameters::CommonName(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::Surname(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::SerialNumber(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::CountryName(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::LocalityName(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::StateOrProvinceName(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::StreetName(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::OrganizationName(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::OrganizationalUnitName(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::EmailAddress(name) => {
                seq.serialize_element(name)?;
            }
            AttributeTypeAndValueParameters::Custom(der) => {
                seq.serialize_element(der)?;
            }
        }
        seq.end()
    }
}

impl<'de> de::Deserialize<'de> for AttributeTypeAndValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = AttributeTypeAndValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded AttributeTypeAndValue")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let ty: ObjectIdentifierAsn1 = seq_next_element!(seq, AttributeTypeAndValue, "type oid");

                let value =
                    match Into::<String>::into(&ty.0).as_str() {
                        oids::AT_COMMON_NAME => AttributeTypeAndValueParameters::CommonName(seq_next_element!(
                            seq,
                            AttributeTypeAndValue,
                            "at common name"
                        )),
                        oids::AT_SURNAME => AttributeTypeAndValueParameters::Surname(seq_next_element!(
                            seq,
                            AttributeTypeAndValue,
                            "at surname"
                        )),
                        oids::AT_SERIAL_NUMBER => AttributeTypeAndValueParameters::SerialNumber(seq_next_element!(
                            seq,
                            AttributeTypeAndValue,
                            "at serial number"
                        )),
                        oids::AT_COUNTRY_NAME => AttributeTypeAndValueParameters::CountryName(seq_next_element!(
                            seq,
                            AttributeTypeAndValue,
                            "at country name"
                        )),
                        oids::AT_LOCALITY_NAME => AttributeTypeAndValueParameters::LocalityName(seq_next_element!(
                            seq,
                            AttributeTypeAndValue,
                            "at locality name"
                        )),
                        oids::AT_STATE_OR_PROVINCE_NAME => AttributeTypeAndValueParameters::StateOrProvinceName(
                            seq_next_element!(seq, AttributeTypeAndValue, "at state or province name"),
                        ),
                        oids::AT_STREET_NAME => AttributeTypeAndValueParameters::StreetName(seq_next_element!(
                            seq,
                            AttributeTypeAndValue,
                            "at street name"
                        )),
                        oids::AT_ORGANIZATION_NAME => AttributeTypeAndValueParameters::OrganizationName(
                            seq_next_element!(seq, AttributeTypeAndValue, "at organization name"),
                        ),
                        oids::AT_ORGANIZATIONAL_UNIT_NAME => AttributeTypeAndValueParameters::OrganizationalUnitName(
                            seq_next_element!(seq, AttributeTypeAndValue, "at organizational unit name"),
                        ),
                        oids::EMAIL_ADDRESS => AttributeTypeAndValueParameters::EmailAddress(seq_next_element!(
                            seq,
                            AttributeTypeAndValue,
                            "at email address"
                        )),
                        _ => AttributeTypeAndValueParameters::Custom(seq_next_element!(
                            seq,
                            AttributeTypeAndValue,
                            "at custom value"
                        )),
                    };

                Ok(AttributeTypeAndValue { ty, value })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}
