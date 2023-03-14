use picky_asn1::bit_string::BitString;
use picky_asn1::restricted_string::{BMPString, CharSetError};
use picky_asn1::tag::{TagClass, TagPeeker};
use picky_asn1::wrapper::{
    BMPStringAsn1, BitStringAsn1, ExplicitContextTag0, ExplicitContextTag1, ExplicitContextTag2, IA5StringAsn1,
    ImplicitContextTag0, ImplicitContextTag1, IntegerAsn1, ObjectIdentifierAsn1, OctetStringAsn1, Optional,
};
use serde::{de, ser, Deserialize, Serialize};
use widestring::U16String;

#[cfg(feature = "ctl")]
use super::ctl::Ctl;

use crate::{oids, DigestInfo};

/// ``` not_rust
/// [RFC 5652 #5.2](https://datatracker.ietf.org/doc/html/rfc5652#section-5.2)
/// EncapsulatedContentInfo ::= SEQUENCE {
///         eContentType ContentType,
///         eContent [0] EXPLICIT OCTET STRING OPTIONAL }
///
///  ContentType ::= OBJECT IDENTIFIER
/// ```
#[derive(Debug, PartialEq, Clone)]
pub enum ContentValue {
    SpcIndirectDataContent(SpcIndirectDataContent),
    OctetString(OctetStringAsn1),
    Data(OctetStringAsn1),
    #[cfg(feature = "ctl")]
    CertificateTrustList(Ctl),
}

impl Serialize for ContentValue {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            ContentValue::SpcIndirectDataContent(spc_indirect_data_content) => {
                spc_indirect_data_content.serialize(serializer)
            }
            ContentValue::OctetString(octet_string) => octet_string.serialize(serializer),
            ContentValue::Data(octet_string) => octet_string.serialize(serializer),
            #[cfg(feature = "ctl")]
            ContentValue::CertificateTrustList(ctl) => ctl.serialize(serializer),
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct EncapsulatedContentInfo {
    pub content_type: ObjectIdentifierAsn1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ExplicitContextTag0<ContentValue>>,
}

impl EncapsulatedContentInfo {
    pub fn new_pkcs7_data(data: Option<Vec<u8>>) -> Self {
        Self {
            content_type: oids::pkcs7().into(),
            content: data.map(|data| ContentValue::Data(data.into()).into()),
        }
    }
}

impl<'de> de::Deserialize<'de> for EncapsulatedContentInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = EncapsulatedContentInfo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded ContentInfo")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let oid: ObjectIdentifierAsn1 =
                    seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;

                let value = match Into::<String>::into(&oid.0).as_str() {
                    oids::SPC_INDIRECT_DATA_OBJID => Some(
                        ContentValue::SpcIndirectDataContent(
                            seq_next_element!(
                                seq,
                                ExplicitContextTag0<SpcIndirectDataContent>,
                                EncapsulatedContentInfo,
                                "SpcIndirectDataContent"
                            )
                            .0,
                        )
                        .into(),
                    ),
                    oids::PKCS7 => seq
                        .next_element::<ExplicitContextTag0<OctetStringAsn1>>()?
                        .map(|value| ExplicitContextTag0(ContentValue::Data(value.0))),
                    #[cfg(feature = "ctl")]
                    oids::CERT_TRUST_LIST => Some(
                        ContentValue::CertificateTrustList(
                            seq_next_element!(
                                seq,
                                ExplicitContextTag0<Ctl>,
                                EncapsulatedContentInfo,
                                "CertificateTrustList"
                            )
                            .0,
                        )
                        .into(),
                    ),
                    _ => Some(ExplicitContextTag0::from(ContentValue::OctetString(
                        seq_next_element!(
                            seq,
                            ExplicitContextTag0<OctetStringAsn1>,
                            EncapsulatedContentInfo,
                            "OctetStringAsn1"
                        )
                        .0,
                    ))),
                };

                Ok(EncapsulatedContentInfo {
                    content_type: oid,
                    content: value,
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

// https://github.com/sassoftware/relic/blob/master/lib/authenticode/structs.go#L46
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SpcSipInfo {
    pub version: IntegerAsn1,
    pub uuid: OctetStringAsn1,
    reserved1: IntegerAsn1,
    reserved2: IntegerAsn1,
    reserved3: IntegerAsn1,
    reserved4: IntegerAsn1,
    reserved5: IntegerAsn1,
}

/// [Authenticode_PE.docx](http://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/Authenticode_PE.docx)
/// ``` not_rust
/// SpcIndirectDataContent ::= SEQUENCE {
///     data                    SpcAttributeTypeAndOptionalValue,
///     messageDigest           DigestInfo
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SpcIndirectDataContent {
    pub data: SpcAttributeAndOptionalValue,
    pub message_digest: DigestInfo,
}

// [Authenticode_PE.docx](http://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/Authenticode_PE.docx)
/// ``` not_rust
/// SpcAttributeTypeAndOptionalValue ::= SEQUENCE {
///     type                    ObjectID,
///     value                   [0] EXPLICIT ANY OPTIONAL
/// }
/// ```
#[derive(Debug, PartialEq, Clone)]
pub enum SpcAttributeAndOptionalValueValue {
    SpcPeImageData(SpcPeImageData),
    SpcSipInfo(SpcSipInfo),
}

impl Serialize for SpcAttributeAndOptionalValueValue {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            SpcAttributeAndOptionalValueValue::SpcPeImageData(spc_pe_image_data) => {
                spc_pe_image_data.serialize(serializer)
            }
            SpcAttributeAndOptionalValueValue::SpcSipInfo(spc_sip_info) => spc_sip_info.serialize(serializer),
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct SpcAttributeAndOptionalValue {
    pub ty: ObjectIdentifierAsn1,
    pub value: SpcAttributeAndOptionalValueValue,
}

impl<'de> de::Deserialize<'de> for SpcAttributeAndOptionalValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SpcAttributeAndOptionalValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded SpcAttributeAndOptionalValue")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let oid: ObjectIdentifierAsn1 = seq_next_element!(seq, SpcAttributeAndOptionalValue, "type oid");

                let value = match Into::<String>::into(&oid.0).as_str() {
                    oids::SPC_PE_IMAGE_DATAOBJ => SpcAttributeAndOptionalValueValue::SpcPeImageData(seq_next_element!(
                        seq,
                        SpcPeImageData,
                        SpcAttributeAndOptionalValue,
                        "a SpcPeImageData object"
                    )),
                    oids::SPC_SIPINFO_OBJID => SpcAttributeAndOptionalValueValue::SpcSipInfo(seq_next_element!(
                        seq,
                        SpcSipInfo,
                        "a SpcSipInfo object"
                    )),
                    _ => {
                        return Err(serde_invalid_value!(
                            SpcAttributeAndOptionalValue,
                            "unknown oid type",
                            "a SPC_PE_IMAGE_DATAOBJ oid"
                        ));
                    }
                };

                Ok(SpcAttributeAndOptionalValue { ty: oid, value })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

/// [Authenticode_PE.docx](http://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/Authenticode_PE.docx)
/// ``` not_rust
/// SpcPeImageData ::= SEQUENCE {
///    flags                   SpcPeImageFlags DEFAULT { includeResources },
///    file                    SpcLink
/// }
/// ```
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct SpcPeImageData {
    pub flags: SpcPeImageFlags,
    pub file: ExplicitContextTag0<SpcLink>, // According to Authenticode_PE.docx, there is  no ExplicitContextTag0, but otherwise created Authenticode signature won't be valid
}

impl<'de> de::Deserialize<'de> for SpcPeImageData {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SpcPeImageData;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded SpcPeImageData")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                Ok(SpcPeImageData {
                    flags: seq.next_element()?.unwrap_or_default(),
                    file: seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?,
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

/// [Authenticode_PE.docx](http://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/Authenticode_PE.docx)
/// ``` not_rust
/// SpcPeImageFlags ::= BIT STRING {
///     includeResources            (0),
///     includeDebugInfo            (1),
///     includeImportAddressTable   (2)
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SpcPeImageFlags(pub BitStringAsn1);

impl Default for SpcPeImageFlags {
    fn default() -> Self {
        let mut flags = BitString::with_len(3);
        flags.set(0, true); // includeResources
        flags.set(1, false); // includeDebugInfo
        flags.set(2, false); // includeImportAddressTable
        Self(flags.into())
    }
}

/// [Authenticode_PE.docx](http://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/Authenticode_PE.docx)
/// ``` not_rust
/// SpcLink ::= CHOICE {
///     url                     [0] IMPLICIT IA5STRING,
///     moniker                 [1] IMPLICIT SpcSerializedObject,
///     file                    [2] EXPLICIT SpcString
/// }
/// ```
#[derive(Debug, PartialEq, Clone)]
pub enum SpcLink {
    Url(Url),
    Moniker(Moniker),
    File(File),
}

impl Serialize for SpcLink {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            SpcLink::Url(url) => url.0.serialize(serializer),
            SpcLink::Moniker(moniker) => moniker.0.serialize(serializer),
            SpcLink::File(file) => file.0.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SpcLink {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SpcLink;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded SpcLink")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let tag_peeker: TagPeeker = seq_next_element!(seq, SpcLink, "choice tag");
                let spc_link = match tag_peeker.next_tag.class_and_number() {
                    (TagClass::ContextSpecific, 0) => SpcLink::Url(Url(seq_next_element!(
                        seq,
                        Optional<ImplicitContextTag0<IA5StringAsn1>>,
                        SpcLink,
                        "Url"
                    ))),
                    (TagClass::ContextSpecific, 1) => SpcLink::Moniker(Moniker(seq_next_element!(
                        seq,
                        Optional<ImplicitContextTag1<SpcSerializedObject>>,
                        SpcLink,
                        "Moniker"
                    ))),
                    (TagClass::ContextSpecific, 2) => SpcLink::File(File(seq_next_element!(
                        seq,
                        ExplicitContextTag2<SpcString>,
                        SpcLink,
                        "File"
                    ))),
                    _ => {
                        return Err(serde_invalid_value!(
                            SpcString,
                            "unknown choice value",
                            "a supported SpcString choice"
                        ))
                    }
                };

                Ok(spc_link)
            }
        }

        deserializer.deserialize_enum("SpcLink", &["Url", "Moniker", "File"], Visitor)
    }
}

impl Default for SpcLink {
    fn default() -> Self {
        SpcLink::File(File::default())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Url(pub Optional<ImplicitContextTag0<IA5StringAsn1>>);

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Moniker(pub Optional<ImplicitContextTag1<SpcSerializedObject>>);

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct File(pub ExplicitContextTag2<SpcString>);

impl Default for File {
    fn default() -> Self {
        let buffer = unicode_string_into_u8_vec(U16String::from_str("<<<Obsolete>>>"));

        File(SpcString::Unicode(Optional(ImplicitContextTag0(BMPString::new(buffer).unwrap().into()))).into())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SpcUuid(pub OctetStringAsn1);

impl Default for SpcUuid {
    fn default() -> Self {
        Self(OctetStringAsn1(vec![
            0xa6, 0xb5, 0x86, 0xd5, 0xb4, 0xa1, 0x24, 0x66, 0xae, 0x05, 0xa2, 0x17, 0xda, 0x8e, 0x60, 0xd6,
        ]))
    }
}

/// [Authenticode_PE.docx](http://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/Authenticode_PE.docx)
/// ``` not_rust
/// SpcSerializedObject ::= SEQUENCE {
///     classId             SpcUuid,
///     serializedData      OCTETSTRING
/// }
///
/// SpcUuid ::= OCTETSTRING
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct SpcSerializedObject {
    pub class_id: SpcUuid,
    pub serialized_data: OctetStringAsn1,
}

/// [Authenticode_PE.docx](http://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/Authenticode_PE.docx)
/// ``` not_rust
/// SpcString ::= CHOICE {
///     unicode                 [0] IMPLICIT BMPSTRING,
///     ascii                   [1] IMPLICIT IA5STRING
/// }
/// ```
#[derive(Debug, PartialEq, Clone)]
pub enum SpcString {
    Unicode(Optional<ImplicitContextTag0<BMPStringAsn1>>),
    Ancii(Optional<ImplicitContextTag1<IA5StringAsn1>>),
}

impl Serialize for SpcString {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            SpcString::Unicode(unicode) => unicode.serialize(serializer),
            SpcString::Ancii(ancii) => ancii.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SpcString {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SpcString;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded SpcString")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let tag_peeker: TagPeeker = seq_next_element!(seq, SpcString, "choice tag");

                let spc_string = match tag_peeker.next_tag.class_and_number() {
                    (TagClass::ContextSpecific, 0) => SpcString::Unicode(seq_next_element!(
                        seq,
                        Optional<ImplicitContextTag0<BMPStringAsn1>>,
                        SpcString,
                        "BMPStringAsn1"
                    )),
                    (TagClass::ContextSpecific, 1) => SpcString::Ancii(seq_next_element!(
                        seq,
                        Optional<ImplicitContextTag1<IA5StringAsn1>>,
                        SpcString,
                        "IA5StringAsn1"
                    )),
                    _ => {
                        return Err(serde_invalid_value!(
                            SpcString,
                            "unknown choice value",
                            "a supported SpcString choice"
                        ));
                    }
                };

                Ok(spc_string)
            }
        }

        deserializer.deserialize_enum("SpcString", &["Unicode, Ancii"], Visitor)
    }
}

impl TryFrom<String> for SpcString {
    type Error = CharSetError;

    fn try_from(string: String) -> Result<Self, Self::Error> {
        let buffer = unicode_string_into_u8_vec(U16String::from_str(&string));

        Ok(SpcString::Unicode(Optional(ImplicitContextTag0(
            BMPString::new(buffer)?.into(),
        ))))
    }
}

/// [Authenticode_PE.docx](http://download.microsoft.com/download/9/c/5/9c5b2167-8017-4bae-9fde-d599bac8184a/Authenticode_PE.docx)
/// ``` not_rust
/// SpcSpOpusInfo ::= SEQUENCE {
///     programName             [0] EXPLICIT SpcString OPTIONAL,
///     moreInfo                [1] EXPLICIT SpcLink OPTIONAL,
/// }
/// ```
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct SpcSpOpusInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_name: Option<ExplicitContextTag0<SpcString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub more_info: Option<ExplicitContextTag1<SpcLink>>,
}

impl<'de> Deserialize<'de> for SpcSpOpusInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SpcSpOpusInfo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded SpcSpOpusInfo")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                Ok(SpcSpOpusInfo {
                    program_name: seq.next_element().unwrap_or(Some(None)).unwrap_or(None),
                    more_info: seq.next_element().unwrap_or(Some(None)).unwrap_or(None),
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

fn unicode_string_into_u8_vec(unicode_string: U16String) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(unicode_string.len() * 2);

    for elem in unicode_string.into_vec().into_iter() {
        let bytes = elem.to_be_bytes();
        buffer.push(bytes[0]);
        buffer.push(bytes[1]);
    }
    buffer
}
