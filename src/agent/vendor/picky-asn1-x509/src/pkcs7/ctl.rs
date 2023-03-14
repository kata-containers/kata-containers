use crate::{oids, AlgorithmIdentifier};
use picky_asn1::wrapper::{
    Asn1SequenceOf, Asn1SetOf, BitStringAsn1, IntegerAsn1, ObjectIdentifierAsn1, OctetStringAsn1,
    OctetStringAsn1Container, UTCTimeAsn1,
};
use serde::{de, ser, Deserialize, Deserializer, Serialize};

/// ``` not_rust
/// CTL ::= SEQUENCE {
///     signers SEQUENCE of OBJECT IDENTIFIER,
///     sequenceNumber INTEGER,
///     effectiveDate UTCTime,
///     digestAlgorithm AlgorithmIdentifier,
///     ctlEntries: SEQUENCE OF CTLEntry
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Ctl {
    pub signers: Asn1SequenceOf<ObjectIdentifierAsn1>,
    pub sequence_number: IntegerAsn1,
    pub effective_date: UTCTimeAsn1,
    pub digest_algorithm: AlgorithmIdentifier,
    pub crl_entries: Asn1SequenceOf<CTLEntry>,
}

/// ``` not_rust
/// CTLEntry ::= SEQUENCE {
///     certFingerprint OCTET STRING,
///     attributes SET OF CTLEntryAttribute
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CTLEntry {
    pub cert_fingerprint: OctetStringAsn1,
    pub attributes: Asn1SetOf<CTLEntryAttribute>,
}

/// ``` not_rust
/// CTLEntryAttribute ::= SEQUENCE {
///     oid OBJECT IDENTIFIER
/// }
/// ```
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct CTLEntryAttribute {
    pub oid: ObjectIdentifierAsn1,
    pub value: CTLEntryAttributeValues,
}

impl<'de> de::Deserialize<'de> for CTLEntryAttribute {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = CTLEntryAttribute;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded CTLEntryAttribute")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let oid: ObjectIdentifierAsn1 =
                    seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let value = match Into::<String>::into(&oid.0).as_str() {
                    oids::CERT_AUTH_ROOT_SHA256_HASH_PROP_ID => CTLEntryAttributeValues::CertAuthRootSha256HashPropId(
                        seq_next_element!(seq, Asn1SetOf<OctetStringAsn1>, CTLEntryAttribute, "OctetStringAsn1"),
                    ),
                    oids::CERT_DISALLOWED_FILETIME_PROP_ID => CTLEntryAttributeValues::CertDisallowedFileTimePropId(
                        seq_next_element!(seq, Asn1SetOf<OctetStringAsn1>, CTLEntryAttribute, "OctetStringAsn1"),
                    ),
                    oids::DISALLOWED_ENHKEY_USAGE => CTLEntryAttributeValues::DisallowedEnhkeyUsage(seq_next_element!(
                        seq,
                        Asn1SetOf<OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>>,
                        CTLEntryAttribute,
                        "OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>"
                    )),
                    oids::CERT_ENHKEY_USAGE_PROP_ID => {
                        CTLEntryAttributeValues::CertEnhkeyUsagePropId(seq_next_element!(
                            seq,
                            Asn1SetOf<OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>>,
                            CTLEntryAttribute,
                            "OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>"
                        ))
                    }
                    oids::CERT_FRIENDLY_NAME_PROP_ID => CTLEntryAttributeValues::CertFriendlyName(seq_next_element!(
                        seq,
                        Asn1SetOf<OctetStringAsn1>,
                        CTLEntryAttribute,
                        "OctetStringAsn1"
                    )),
                    oids::CERT_KEY_IDENTIFIER_PROP_ID => CTLEntryAttributeValues::CertKeyIdentifierPropId(
                        seq_next_element!(seq, Asn1SetOf<OctetStringAsn1>, CTLEntryAttribute, "OctetStringAsn1"),
                    ),
                    oids::CERT_ROOT_PROGRAM_CHAIN_POLICIES_PROP_ID => {
                        CTLEntryAttributeValues::CertRootProgramChainPolicy(seq_next_element!(
                            seq,
                            Asn1SetOf<OctetStringAsn1Container<RootProgramChainPolicy>>,
                            CTLEntryAttribute,
                            "OctetStringAsn1Container<RootProgramChainPolicy>"
                        ))
                    }
                    oids::CERT_ROOT_PROGRAM_CERT_POLICIES_PROP_ID => {
                        CTLEntryAttributeValues::CertRootProgramCertPoliciesPropId(seq_next_element!(
                            seq,
                            Asn1SetOf<OctetStringAsn1Container<Asn1SequenceOf<CertPolicy>>>,
                            CTLEntryAttribute,
                            "OctetStringAsn1Container<CertPolicies>"
                        ))
                    }
                    oids::CERT_SUBJECT_NAME_MD5_HASH_PROP_ID => CTLEntryAttributeValues::CertSubjectNameMd5HashPropId(
                        seq_next_element!(seq, Asn1SetOf<OctetStringAsn1>, CTLEntryAttribute, "OctetStringAsn1"),
                    ),
                    oids::UNKNOWN_RESERVED_PROP_ID_126 => CTLEntryAttributeValues::UnknownReservedPropId126(
                        seq_next_element!(seq, Asn1SetOf<OctetStringAsn1>, CTLEntryAttribute, "OctetStringAsn1"),
                    ),
                    oids::UNKNOWN_RESERVED_PROP_ID_127 => {
                        CTLEntryAttributeValues::UnknownReservedPropId127(seq_next_element!(
                            seq,
                            Asn1SetOf<OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>>,
                            CTLEntryAttribute,
                            "OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>"
                        ))
                    }
                    _ => {
                        return Err(serde_invalid_value!(
                            CTLEntryAttribute,
                            "unknown oid typ",
                            "any of [CERT_AUTH_ROOT_SHA256_HASH_PROP_ID, CERT_DISALLOWED_FILETIME_PROP_ID, DISALLOWED_ENHKEY_USAGE, CERT_ENHKEY_USAGE_PROP_ID,\
                             CERT_FRIENDLY_NAME_PROP_ID, CERT_KEY_IDENTIFIER_PROP_ID, CERT_ROOT_PROGRAM_CHAIN_POLICIES_PROP_ID, CERT_ROOT_PROGRAM_CERT_POLICIES_PROP_ID, \
                             CERT_SUBJECT_NAME_MD5_HASH_PROP_ID, UNKNOWN_RESERVED_PROP_ID_126, UNKNOWN_RESERVED_PROP_ID_127]"
                        ));
                    }
                };

                Ok(CTLEntryAttribute { oid, value })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CTLEntryAttributeValues {
    CertAuthRootSha256HashPropId(Asn1SetOf<OctetStringAsn1>),
    CertDisallowedFileTimePropId(Asn1SetOf<OctetStringAsn1>), // A 64-bit little-endian Windows FILETIME that indicates when the certificate was revoked. It can be empty, which indicates since epoch
    DisallowedEnhkeyUsage(Asn1SetOf<OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>>),
    CertEnhkeyUsagePropId(Asn1SetOf<OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>>), // Contains an array of object identifiers (OIDs) for Certificate Trust List (CTL) extensions
    CertFriendlyName(Asn1SetOf<OctetStringAsn1>), // The certificate friendly name
    CertKeyIdentifierPropId(Asn1SetOf<OctetStringAsn1>), // The name of the private key file
    CertRootProgramChainPolicy(Asn1SetOf<OctetStringAsn1Container<RootProgramChainPolicy>>),
    CertRootProgramCertPoliciesPropId(Asn1SetOf<OctetStringAsn1Container<Asn1SequenceOf<CertPolicy>>>),
    CertSubjectNameMd5HashPropId(Asn1SetOf<OctetStringAsn1>), // Contains the md5 hash of the subject name
    UnknownReservedPropId126(Asn1SetOf<OctetStringAsn1>), // Indicates the NotBefore time of a particular certificate, as a Windows FILETIME
    UnknownReservedPropId127(Asn1SetOf<OctetStringAsn1Container<Asn1SequenceOf<ObjectIdentifierAsn1>>>), // Appears to be the set of EKUs for which the NotBefore-ing applies
}

impl Serialize for CTLEntryAttributeValues {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            CTLEntryAttributeValues::CertAuthRootSha256HashPropId(octet_string) => octet_string.serialize(serializer),
            CTLEntryAttributeValues::CertDisallowedFileTimePropId(octet_string) => octet_string.serialize(serializer),
            CTLEntryAttributeValues::DisallowedEnhkeyUsage(octet_string_container) => {
                octet_string_container.serialize(serializer)
            }
            CTLEntryAttributeValues::CertEnhkeyUsagePropId(octet_string_container) => {
                octet_string_container.serialize(serializer)
            }
            CTLEntryAttributeValues::CertFriendlyName(octet_string) => octet_string.serialize(serializer),
            CTLEntryAttributeValues::CertKeyIdentifierPropId(octet_string) => octet_string.serialize(serializer),
            CTLEntryAttributeValues::CertRootProgramChainPolicy(root_program_chain_policy) => {
                root_program_chain_policy.serialize(serializer)
            }
            CTLEntryAttributeValues::CertRootProgramCertPoliciesPropId(octet_string_container) => {
                octet_string_container.serialize(serializer)
            }
            CTLEntryAttributeValues::CertSubjectNameMd5HashPropId(octet_string) => octet_string.serialize(serializer),
            CTLEntryAttributeValues::UnknownReservedPropId126(octet_string) => octet_string.serialize(serializer),
            CTLEntryAttributeValues::UnknownReservedPropId127(octet_string_container) => {
                octet_string_container.serialize(serializer)
            }
        }
    }
}

/// ``` not_rust
/// RootProgramChainPolicy ::= SEQUENCE {
///     oid OBJECT IDENTIFIER
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RootProgramChainPolicy {
    pub oid: ObjectIdentifierAsn1,
}

/// ``` not_rust
/// CertPolicy ::= SEQUENCE {
///     oid  OBJECT IDENTIFIER,
///     qualifier SEQUENCE OF PolicyQualifier
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct CertPolicy {
    pub oid: ObjectIdentifierAsn1,
    pub qualifier: Asn1SequenceOf<PolicyQualifier>,
}

/// ``` not_rust
/// PolicyQualifier ::= SEQUENCE {
///     oid OBJECT IDENTIFIER,
///     bits BIT STRING
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PolicyQualifier {
    pub oid: ObjectIdentifierAsn1,
    pub bits: BitStringAsn1,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AlgorithmIdentifier, ShaVariant};
    use picky_asn1::date::UTCTime;

    #[test]
    fn decode_certificate_trust_list_entry() {
        let ctl_entry_hex = base64::decode(
            "MIIBKgQUzdTurmAArH9Aw4AsFx4wFIAwwHIxggEQMBgGCisGAQQBgjcKC34xCgQI\
                                 AADZtUTB0gEwHgYKKwYBBAGCNwoLaTEQBA4wDAYKKwYBBAGCNzwDAjAgBgorBgEE\
                                 AYI3CgsdMRIEEPDEAvBATqmtvyWgPd8spvowJAYKKwYBBAGCNwoLFDEWBBQOrIJg\
                                 QFYnl+UlE/wq4QpTlVnkpDAwBgorBgEEAYI3CgtiMSIEIIhd5kw0Dj6nBljwHhFF\
                                 +Vf82ieqvuoaufqp/bAQLUB3MFoGCisGAQQBgjcKCwsxTARKTQBpAGMAcgBvAHMA\
                                 bwBmAHQAIABSAG8AbwB0ACAAQwBlAHIAdABpAGYAaQBjAGEAdABlACAAQQB1AHQA\
                                 aABvAHIAaQB0AHkAAAA=",
        )
        .unwrap();

        let cert_fingerprint = OctetStringAsn1::from(ctl_entry_hex[6..26].to_vec());

        let unknown_reserved_prop_id_126_entry = CTLEntryAttribute {
            oid: oids::unknown_reserved_prop_id_126().into(),
            value: CTLEntryAttributeValues::UnknownReservedPropId126(
                vec![ctl_entry_hex[48..56].to_vec().into()].into(),
            ),
        };
        check_serde!(unknown_reserved_prop_id_126_entry: CTLEntryAttribute in ctl_entry_hex[30..56]);

        let cert_root_program_chain_policies_prop_id_entry = CTLEntryAttribute {
            oid: oids::cert_root_program_chain_policies_prop_id().into(),
            value: CTLEntryAttributeValues::CertRootProgramChainPolicy(
                vec![RootProgramChainPolicy {
                    oid: oids::auto_update_end_revocation().into(),
                }
                .into()]
                .into(),
            ),
        };
        check_serde!(cert_root_program_chain_policies_prop_id_entry: CTLEntryAttribute in ctl_entry_hex[56..88]);

        let cert_subject_name_md5_hash_prop_id_entry = CTLEntryAttribute {
            oid: oids::cert_subject_name_md5_hash_prop_id().into(),
            value: CTLEntryAttributeValues::CertSubjectNameMd5HashPropId(
                vec![ctl_entry_hex[106..122].to_vec().into()].into(),
            ),
        };
        check_serde!(cert_subject_name_md5_hash_prop_id_entry: CTLEntryAttribute in ctl_entry_hex[88..122]);

        let cert_key_identifier_prop_id_entry = CTLEntryAttribute {
            oid: oids::cert_key_identifier_prop_id().into(),
            value: CTLEntryAttributeValues::CertKeyIdentifierPropId(
                vec![ctl_entry_hex[140..160].to_vec().into()].into(),
            ),
        };
        check_serde!(cert_key_identifier_prop_id_entry: CTLEntryAttribute in ctl_entry_hex[122..160]);

        let cert_auto_root_sha256_hash_prop_id_entry = CTLEntryAttribute {
            oid: oids::cert_auto_root_sha256_hash_prop_id().into(),
            value: CTLEntryAttributeValues::CertAuthRootSha256HashPropId(
                vec![ctl_entry_hex[178..210].to_vec().into()].into(),
            ),
        };
        check_serde!(cert_auto_root_sha256_hash_prop_id_entry: CTLEntryAttribute in ctl_entry_hex[160..210]);

        let cert_friendly_name_prop_id_entry = CTLEntryAttribute {
            oid: oids::cert_friendly_name_prop_id().into(),
            value: CTLEntryAttributeValues::CertFriendlyName(vec![ctl_entry_hex[228..302].to_vec().into()].into()),
        };
        check_serde!(cert_friendly_name_prop_id_entry: CTLEntryAttribute in ctl_entry_hex[210..302]);

        let attributes = Asn1SetOf::from(vec![
            unknown_reserved_prop_id_126_entry,
            cert_root_program_chain_policies_prop_id_entry,
            cert_subject_name_md5_hash_prop_id_entry,
            cert_key_identifier_prop_id_entry,
            cert_auto_root_sha256_hash_prop_id_entry,
            cert_friendly_name_prop_id_entry,
        ]);

        let ctl_entry = CTLEntry {
            cert_fingerprint,
            attributes,
        };

        check_serde!(ctl_entry: CTLEntry in ctl_entry_hex);
    }

    #[test]
    fn decode_certificate_trust_list() {
        let ctl_hex = base64::decode(
            "MIIBZzAMBgorBgEEAYI3CgMJAgkUAddM40UqdcsXDTIxMDUxOTE5MTUwM1owCQYF\
            Kw4DAhoFADCCATAwggEsBBQY98H8wwkCA/1bqi+GGnVJdsjdJTGCARIwGAYKKwYB\
            BAGCNwoLaDEKBAgAADYETd/TATAYBgorBgEEAYI3Cgt+MQoECAAAEMUektIBMBwG\
            CisGAQQBgjcKCwkxDgQMMAoGCCsGAQUFBwMIMCAGCisGAQQBgjcKCx0xEgQQT/fb\
            VtD9iKGf47qLwA4GYjAkBgorBgEEAYI3CgsUMRYEFD7fKQzB9cxzLOs9JOF+Utq9\
            J+LwMDAGCisGAQQBgjcKC2IxIgQgW3iZh/PEBVuHAJQbM3g6Xxbgz/k36jIBH+BH\
            efdjUwgwRAYKKwYBBAGCNwoLCzE2BDRWAGUAcgBpAFMAaQBnAG4AIABUAGkAbQBl\
            ACAAUwB0AGEAbQBwAGkAbgBnACAAQwBBAAAA",
        )
        .unwrap();

        let cert_enhkey_usage_prop_id_entry = CTLEntryAttribute {
            oid: oids::cert_enhkey_usage_prop_id().into(),
            value: CTLEntryAttributeValues::CertEnhkeyUsagePropId(
                vec![OctetStringAsn1Container(vec![oids::kp_time_stamping().into()].into())].into(),
            ),
        };

        check_serde!(cert_enhkey_usage_prop_id_entry: CTLEntryAttribute in ctl_hex[141..171]);

        let ctl = Ctl {
            signers: vec![oids::root_list_signer().into()].into(),
            sequence_number: ctl_hex[20..29].to_vec().into(),
            effective_date: UTCTime::new(2021, 5, 19, 19, 15, 3).unwrap().into(),
            digest_algorithm: AlgorithmIdentifier::new_sha(ShaVariant::SHA1),
            crl_entries: vec![CTLEntry {
                cert_fingerprint: ctl_hex[65..85].to_vec().into(),
                attributes: vec![
                    CTLEntryAttribute {
                        oid: oids::cert_disallowed_filetime_prop_id().into(),
                        value: CTLEntryAttributeValues::CertDisallowedFileTimePropId(
                            vec![ctl_hex[107..115].to_vec().into()].into(),
                        ),
                    },
                    CTLEntryAttribute {
                        oid: oids::unknown_reserved_prop_id_126().into(),
                        value: CTLEntryAttributeValues::UnknownReservedPropId126(
                            vec![ctl_hex[133..141].to_vec().into()].into(),
                        ),
                    },
                    cert_enhkey_usage_prop_id_entry,
                    CTLEntryAttribute {
                        oid: oids::cert_subject_name_md5_hash_prop_id().into(),
                        value: CTLEntryAttributeValues::CertSubjectNameMd5HashPropId(
                            vec![ctl_hex[189..205].to_vec().into()].into(),
                        ),
                    },
                    CTLEntryAttribute {
                        oid: oids::cert_key_identifier_prop_id().into(),
                        value: CTLEntryAttributeValues::CertKeyIdentifierPropId(
                            vec![ctl_hex[223..243].to_vec().into()].into(),
                        ),
                    },
                    CTLEntryAttribute {
                        oid: oids::cert_auto_root_sha256_hash_prop_id().into(),
                        value: CTLEntryAttributeValues::CertAuthRootSha256HashPropId(
                            vec![ctl_hex[261..293].to_vec().into()].into(),
                        ),
                    },
                    CTLEntryAttribute {
                        oid: oids::cert_friendly_name_prop_id().into(),
                        value: CTLEntryAttributeValues::CertFriendlyName(vec![ctl_hex[311..].to_vec().into()].into()),
                    },
                ]
                .into(),
            }]
            .into(),
        };

        check_serde!(ctl: Ctl in ctl_hex);
    }
}
