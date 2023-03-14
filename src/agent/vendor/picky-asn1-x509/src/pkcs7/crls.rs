use super::signer_info::CertificateSerialNumber;
use crate::{AlgorithmIdentifier, Extensions, Name, Time, Version};
use picky_asn1::tag::{Tag, TagClass, TagPeeker};
use picky_asn1::wrapper::{
    Asn1SequenceOf, BitStringAsn1, ExplicitContextTag0, ImplicitContextTag1, ObjectIdentifierAsn1,
};
use serde::{de, ser, Deserialize, Serialize};

/// [RFC 5652 #10.2.1](https://datatracker.ietf.org/doc/html/rfc5652#section-10.2.1)
///
/// ```not_rust
/// RevocationInfoChoices ::= SET OF RevocationInfoChoice
/// ```
#[derive(Debug, PartialEq, Clone, Default)]
pub struct RevocationInfoChoices(pub Vec<RevocationInfoChoice>);

// This is a workaround for constructed encoding as implicit

impl ser::Serialize for RevocationInfoChoices {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        let mut raw_der = picky_asn1_der::to_vec(&self.0).unwrap_or_else(|_| vec![0]);
        raw_der[0] = Tag::context_specific_constructed(1).inner();
        picky_asn1_der::Asn1RawDer(raw_der).serialize(serializer)
    }
}

impl<'de> de::Deserialize<'de> for RevocationInfoChoices {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        let mut raw_der = picky_asn1_der::Asn1RawDer::deserialize(deserializer)?.0;
        raw_der[0] = Tag::SEQUENCE.inner();
        let vec = picky_asn1_der::from_bytes(&raw_der).unwrap_or_default();
        Ok(RevocationInfoChoices(vec))
    }
}

/// [RFC 5652 #10.2.1](https://datatracker.ietf.org/doc/html/rfc5652#section-10.2.1)
///
/// ```not_rust
/// RevocationInfoChoice ::= CHOICE {
///    crl CertificateList,
///    other [1] IMPLICIT OtherRevocationInfoFormat }
///
/// ```
#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq, Clone)]
pub enum RevocationInfoChoice {
    Crl(CertificateList),
    Other(ImplicitContextTag1<OtherRevocationInfoFormat>),
}

impl ser::Serialize for RevocationInfoChoice {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            RevocationInfoChoice::Crl(certificate_list) => certificate_list.serialize(serializer),
            RevocationInfoChoice::Other(other_revocation_info_format) => {
                other_revocation_info_format.serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for RevocationInfoChoice {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = RevocationInfoChoice;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded RevocationInfoChoice")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let tag_peeker: TagPeeker = seq_next_element!(seq, RevocationInfoChoice, "choice tag");

                let revocation_info_choice =
                    if tag_peeker.next_tag.class() == TagClass::ContextSpecific && tag_peeker.next_tag.number() == 1 {
                        RevocationInfoChoice::Other(seq_next_element!(
                            seq,
                            RevocationInfoChoice,
                            "OtherRevocationInfoFormat "
                        ))
                    } else {
                        RevocationInfoChoice::Crl(seq_next_element!(seq, RevocationInfoChoice, "CertificateList"))
                    };

                Ok(revocation_info_choice)
            }
        }

        deserializer.deserialize_enum("RevocationInfoChoice", &["Crl", "Other"], Visitor)
    }
}

/// CRLs are specified in X.509
/// ``` not_rust
/// [RFC 5280 #5.1](https://datatracker.ietf.org/doc/html/rfc5280#section-5.1)
/// CertificateList  ::=  SEQUENCE  {
///         tbsCertList          TBSCertList,
///         signatureAlgorithm   AlgorithmIdentifier,
///         signatureValue       BIT STRING  }
///
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CertificateList {
    pub tbs_cert_list: TbsCertList,
    pub signature_algorithm: AlgorithmIdentifier,
    pub signature_value: BitStringAsn1,
}

/// ``` not_rust
/// [RFC 5280 #5.1](https://datatracker.ietf.org/doc/html/rfc5280#section-5.1)
///  TBSCertList  ::=  SEQUENCE  {
///         version                 Version OPTIONAL,
///                                      -- if present, MUST be v2
///         signature               AlgorithmIdentifier,
///         issuer                  Name,
///         thisUpdate              Time,
///         nextUpdate              Time OPTIONAL,
///         revokedCertificates     SEQUENCE OF SEQUENCE  {
///              userCertificate         CertificateSerialNumber,
///              revocationDate          Time,
///              crlEntryExtensions      Extensions OPTIONAL
///                                       -- if present, version MUST be v2
///                                   }  OPTIONAL,
///         crlExtensions           [0]  EXPLICIT Extensions OPTIONAL
///                                       -- if present, version MUST be v2
///                                 }
/// ```
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct TbsCertList {
    pub version: Option<Version>,
    pub signature: AlgorithmIdentifier,
    pub issuer: Name,
    pub this_update: Time,
    pub next_update: Option<Time>,
    pub revoked_certificates: Option<RevokedCertificates>,
    pub crl_extension: ExplicitContextTag0<Option<Extensions>>,
}

impl<'de> de::Deserialize<'de> for TbsCertList {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = TbsCertList;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded TbsCertList")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let version = seq.next_element().unwrap_or(Some(None)).unwrap_or(None);
                if version.is_some() && !version.eq(&Some(Version::V2)) {
                    return Err(serde_invalid_value!(
                        TbsCertList,
                        "Version of TbsCertList doesn't equal to v2",
                        "Version of TbsCertList equals to v2"
                    ));
                }

                let signature = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let issuer = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let this_update = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let next_update = seq.next_element().unwrap_or(Some(None)).unwrap_or(None);

                let tag_peeker: TagPeeker = seq_next_element!(seq, TbsCertList, "a tag");
                let revoked_certificates = if tag_peeker.next_tag == Tag::SEQUENCE {
                    seq.next_element()?.ok_or_else(|| de::Error::invalid_length(5, &self))?
                } else {
                    None
                };

                let crl_extension = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(6, &self))?;

                Ok(TbsCertList {
                    version,
                    signature,
                    issuer,
                    this_update,
                    next_update,
                    revoked_certificates,
                    crl_extension,
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct RevokedCertificates(pub Asn1SequenceOf<RevokedCertificate>);

#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct RevokedCertificate {
    pub user_certificate: CertificateSerialNumber,
    pub revocation_data: Time,
    pub crl_entry_extensions: Option<Extensions>,
}

impl<'de> de::Deserialize<'de> for RevokedCertificate {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = RevokedCertificate;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-decoded TbsCertList")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                Ok(RevokedCertificate {
                    user_certificate: seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?,
                    revocation_data: seq.next_element()?.ok_or_else(|| de::Error::invalid_length(2, &self))?,
                    crl_entry_extensions: seq.next_element().unwrap_or(Some(None)).unwrap_or(None),
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

/// [RFC 5652 #10.2.1](https://datatracker.ietf.org/doc/html/rfc5652#section-10.2.1)
/// ``` not_rust
/// OtherRevocationInfoFormat ::= SEQUENCE {
///    otherRevInfoFormat OBJECT IDENTIFIER,
///    otherRevInfo ANY DEFINED BY otherRevInfoFormat }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OtherRevocationInfoFormat {
    other_rev_info_format: ObjectIdentifierAsn1,
    other_rev_info: (),
}
