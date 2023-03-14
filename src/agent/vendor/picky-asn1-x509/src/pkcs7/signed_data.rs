use super::content_info::EncapsulatedContentInfo;
use super::crls::RevocationInfoChoices;
use super::signer_info::SignerInfo;
use crate::cmsversion::CmsVersion;
use crate::AlgorithmIdentifier;
use picky_asn1::tag::{Tag, TagClass, TagPeeker};
use picky_asn1::wrapper::{Asn1SetOf, Optional};
use picky_asn1_der::Asn1RawDer;
use serde::{de, ser, Deserialize, Serialize};

/// [RFC 5652 #5.1](https://datatracker.ietf.org/doc/html/rfc5652#section-5.1)
/// ``` not_rust
/// SignedData ::= SEQUENCE {
///         version CMSVersion,
///         digestAlgorithms DigestAlgorithmIdentifiers,
///         encapContentInfo EncapsulatedContentInfo,
///         certificates [0] IMPLICIT CertificateSet OPTIONAL,
///         crls [1] IMPLICIT RevocationInfoChoices OPTIONAL,
///         signerInfos SignerInfos }
/// ```
#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct SignedData {
    pub version: CmsVersion,
    pub digest_algorithms: DigestAlgorithmIdentifiers,
    pub content_info: EncapsulatedContentInfo,
    pub certificates: Optional<CertificateSet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crls: Option<RevocationInfoChoices>,
    pub signers_infos: SignersInfos,
}

// Implement Deserialize manually to support absent RevocationInfoChoices
impl<'de> de::Deserialize<'de> for SignedData {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SignedData;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded SignedData")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let version = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let digest_algorithms = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let content_info = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let certificates = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(3, &self))?;

                let tag_peeker: TagPeeker = seq_next_element!(seq, SignedData, "ApplicationTag1");
                let crls =
                    if tag_peeker.next_tag.class() == TagClass::ContextSpecific && tag_peeker.next_tag.number() == 1 {
                        seq.next_element()?.ok_or_else(|| de::Error::invalid_length(4, &self))?
                    } else {
                        None
                    };

                let signers_infos = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(5, &self))?;

                Ok(SignedData {
                    version,
                    digest_algorithms,
                    content_info,
                    certificates,
                    crls,
                    signers_infos,
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

/// [RFC 5652 #5.1](https://datatracker.ietf.org/doc/html/rfc5652#section-5.1)
/// ``` not_rust
/// DigestAlgorithmIdentifiers ::= SET OF DigestAlgorithmIdentifier
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DigestAlgorithmIdentifiers(pub Asn1SetOf<AlgorithmIdentifier>);

/// [RFC 5652 #5.1](https://datatracker.ietf.org/doc/html/rfc5652#section-5.1)
/// ``` not_rust
/// SignerInfos ::= SET OF SignerInfo
/// ```
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SignersInfos(pub Asn1SetOf<SignerInfo>);

/// [RFC 5652 #10.2.3](https://datatracker.ietf.org/doc/html/rfc5652#section-10.2.3)
/// ``` not_rust
/// CertificateSet ::= SET OF CertificateChoices
/// ```
#[derive(Debug, PartialEq, Clone, Default)]
pub struct CertificateSet(pub Vec<CertificateChoices>);

// This is a workaround for constructed encoding as implicit

impl ser::Serialize for CertificateSet {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        let mut raw_der = picky_asn1_der::to_vec(&self.0).unwrap_or_else(|_| vec![0]);
        raw_der[0] = Tag::context_specific_constructed(0).inner();
        picky_asn1_der::Asn1RawDer(raw_der).serialize(serializer)
    }
}

impl<'de> de::Deserialize<'de> for CertificateSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        let mut raw_der = picky_asn1_der::Asn1RawDer::deserialize(deserializer)?.0;
        raw_der[0] = Tag::SEQUENCE.inner();
        let vec = picky_asn1_der::from_bytes(&raw_der).unwrap_or_default();
        Ok(CertificateSet(vec))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CertificateChoices {
    Certificate(Asn1RawDer),
    Other(Asn1RawDer),
}

impl Serialize for CertificateChoices {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match &self {
            CertificateChoices::Certificate(certificate) => certificate.serialize(serializer),
            CertificateChoices::Other(other) => other.serialize(serializer),
        }
    }
}

impl<'de> de::Deserialize<'de> for CertificateChoices {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = CertificateChoices;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded CertificateChoices")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let tag_peeker: TagPeeker = seq_next_element!(seq, CertificateChoices, "Any tag");
                let certificate_choice = match tag_peeker.next_tag {
                    Tag::SEQUENCE => {
                        CertificateChoices::Certificate(seq_next_element!(seq, CertificateChoices, "Certificate"))
                    }
                    _ => {
                        CertificateChoices::Other(seq_next_element!(seq, CertificateChoices, "Other certificate type"))
                    }
                };

                Ok(certificate_choice)
            }
        }

        deserializer.deserialize_enum("CertificateChoices", &["Certificate, Other"], Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crls::*;
    use crate::{
        oids, Certificate, EncapsulatedRsaPublicKey, Extension, Extensions, KeyIdentifier, Name, NameAttr, PublicKey,
        RsaPublicKey, SubjectPublicKeyInfo, TbsCertificate, Validity, Version,
    };
    use picky_asn1::bit_string::BitString;
    use picky_asn1::date::UTCTime;
    use picky_asn1::restricted_string::{IA5String, PrintableString};
    use picky_asn1::wrapper::{IntegerAsn1, ObjectIdentifierAsn1, OctetStringAsn1Container, PrintableStringAsn1};

    #[test]
    fn decode_test() {
        let pkcs7 = base64::decode(
            "MIIGOgYJKoZIhvcNAQcCoIIGKzCCBicCAQExADALBgkqhkiG9w0BBwGgggYNMIIG\
                CTCCA/GgAwIBAgIUOnS/zC1zk2aJttmSVNtzX8rhMXwwDQYJKoZIhvcNAQELBQAw\
                gZMxCzAJBgNVBAYTAlVBMRIwEAYDVQQIDAlIdW1ibGVHdXkxETAPBgNVBAcMCFNv\
                bWVDaXR5MRkwFwYDVQQKDBBTb21lT3JnYW5pemF0aW9uMREwDwYDVQQLDAhTb21l\
                VW5pdDEMMAoGA1UEAwwDR3V5MSEwHwYJKoZIhvcNAQkBFhJzb21lZW1haWxAbWFp\
                bC5jb20wHhcNMjEwNDIzMTQzMzQzWhcNMjIwNDIzMTQzMzQzWjCBkzELMAkGA1UE\
                BhMCVUExEjAQBgNVBAgMCUh1bWJsZUd1eTERMA8GA1UEBwwIU29tZUNpdHkxGTAX\
                BgNVBAoMEFNvbWVPcmdhbml6YXRpb24xETAPBgNVBAsMCFNvbWVVbml0MQwwCgYD\
                VQQDDANHdXkxITAfBgkqhkiG9w0BCQEWEnNvbWVlbWFpbEBtYWlsLmNvbTCCAiIw\
                DQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAM6JtTiGxVFdQr0r5hpBioObluoZ\
                UW/u4RMPLlb4xwuIVM+q7Xk968c8FKoxMsTGPfjfF6CBHhvcZTojYRLqFdHaYRzl\
                +m5gnR6ZJRYGOtH7dyFX+2UgTIuLxsBPoXoY/DICpUp2sch8eXmi+lwL1A8Kk9pM\
                CB0s0+nVwNLqpa6aZg5kFkverZzn8tdV8z2yg/BV1fx7FGIDYFuoqc10azEg9aa8\
                bq1psf4c4IrymFEBvuXlvi/vukY/hUPFLHDAjt6vQeDNT0GjsPIj7Fb5ISLEVbBb\
                qMKq0Atr6Af2avtIMudTVm+BT9QlX1gUr83GLiIhsPbS/WBPJcdeWLxvjWIUIJNo\
                hIJkL6YhYhkeniNN5Pq0zrhkSGNt5ai2ZeW/D4npEeCbR7gjsQm8LPJDrnDH3Iax\
                KvPgxen/rCMfssgw6UUWUEGn3n6QPtBp7HcWe+oBQOEuL6zIJKG8XzEypn6EZm+x\
                p7TjCcUgRm1X5OtDnc8E8yHsrs9dKLhzLARs6XDgcw1KhfhzryLY6VsjZD9mm5iu\
                PVgw0Hg+v4cxekWYcjJWCf6EjsCV9iax4UwGb1G7yD5XsYULajZOYqRYNak2Jruu\
                18daA66TQ8HNas25YFFQQQtQG/1RrL1u853DBlrxaNcZQfR6mkE1D7O5MUADqjoM\
                pgoL7k2XqkJMjs/PAgMBAAGjUzBRMB0GA1UdDgQWBBQAX8F1PgwVwxbjCDdpvYKI\
                0YW9DTAfBgNVHSMEGDAWgBQAX8F1PgwVwxbjCDdpvYKI0YW9DTAPBgNVHRMBAf8E\
                BTADAQH/MA0GCSqGSIb3DQEBCwUAA4ICAQAH3Qweqt8+PnQoAKTPXTUMp+lE01a0\
                2vzoc7EPWiclyuQPatlUIyUEH5nbBiXu8v9X5wHIrfzkV7tO+clVy9w6a4Fhnejv\
                2zurSHf6vP/UEq2gPuBJ1jc1BDpE4TtlZdrO6GYBQwETRBbw44lFvyk6sjnCHPgz\
                nl5dryWIyNSALFpSzUJ9xSdtzWEKnWe9NaxBc6b0RxJSsRl33Fx25WkKMuhY4j26\
                wZvWMSj86eRdI7BP31UGEt8GdfQscz5JtMlY+eJbilAMTZt4iAEJFv9OI7/asVJv\
                u8oNZJewGstWqRyRrJcHeEINjxeKL0quKJQF38fCd6pqRI7PlPBaGfVCSHTggpKO\
                yD0ACcE13kcjnOwa8J/DFFZVpI3oofGUE+hajJT09vGJv4NJKUfdJIuieEFeJe8B\
                TPkVjCHp6j6Vj56EdGqvkYtVsuzHUNlIsEcpXGEiODbwbps7GxPCiurVIldun2Gu\
                1mq8Q6aU+yh5Fs5ZsSXozzXyWqwPkT5WbJEOAUMd2+JSRHN83MOSqq+igpDBKQZQ\
                t5vcoqFzuspOVIvdPLFY3pPZY9dxVNdDi4T6qJNZCq++Ukyc0LQOUkshF9HaHB3I\
                xUDGjR5n4X0lkjgM5IvL+OaZREqWkD/tiCu4V/5Z86mZi6VwCcgYrp/Q4bFjsWBw\
                p0mAUFZ9UjurAaEAMQA=",
        )
        .unwrap();

        assert_eq!(CmsVersion::V1, CmsVersion::from_u8(*pkcs7.get(25).unwrap()).unwrap());

        let digest_algorithm_identifiers = DigestAlgorithmIdentifiers(vec![].into());

        check_serde!(digest_algorithm_identifiers: DigestAlgorithmIdentifiers in pkcs7[26..28]);

        let content_info = EncapsulatedContentInfo {
            content_type: ObjectIdentifierAsn1::from(oids::pkcs7()),
            content: None,
        };

        check_serde!(content_info: EncapsulatedContentInfo in pkcs7[28..41]);

        let mut issuer = Name::new();
        issuer.add_attr(
            NameAttr::CountryName,
            PrintableStringAsn1::from(PrintableString::new("UA".as_bytes()).unwrap()),
        );
        issuer.add_attr(NameAttr::StateOrProvinceName, "HumbleGuy");
        issuer.add_attr(NameAttr::LocalityName, "SomeCity");
        issuer.add_attr(NameAttr::OrganizationName, "SomeOrganization");
        issuer.add_attr(NameAttr::OrganizationalUnitName, "SomeUnit");
        issuer.add_attr(NameAttr::CommonName, "Guy");
        issuer.add_email(IA5String::new("someemail@mail.com".as_bytes()).unwrap());

        check_serde!(issuer: Name in pkcs7[95..245]);

        let validity = Validity {
            not_before: UTCTime::new(2021, 4, 23, 14, 33, 43).unwrap().into(),
            not_after: UTCTime::new(2022, 4, 23, 14, 33, 43).unwrap().into(),
        };

        check_serde!(validity: Validity in pkcs7[245..277]);

        let subject = issuer.clone();

        check_serde!(subject: Name in pkcs7[277..427]);

        let subject_public_key_info = SubjectPublicKeyInfo {
            algorithm: AlgorithmIdentifier::new_rsa_encryption(),
            subject_public_key: PublicKey::Rsa(EncapsulatedRsaPublicKey::from(RsaPublicKey {
                modulus: IntegerAsn1::from(pkcs7[459..972].to_vec()),
                public_exponent: IntegerAsn1::from(pkcs7[974..977].to_vec()),
            })),
        };

        check_serde!(subject_public_key_info: SubjectPublicKeyInfo in pkcs7[427..977]);

        let extensions = Extensions(vec![
            Extension::new_subject_key_identifier(pkcs7[992..1012].to_vec()),
            Extension::new_authority_key_identifier(KeyIdentifier::from(pkcs7[1025..1045].to_vec()), None, None),
            Extension::new_basic_constraints(*pkcs7.get(1054).unwrap() != 0, None),
        ]);

        check_serde!(extensions: Extensions in  pkcs7[979..1062]);

        let full_certificate = Certificate {
            tbs_certificate: TbsCertificate {
                version: Version::V3.into(),
                serial_number: IntegerAsn1(pkcs7[60..80].to_vec()),
                signature: AlgorithmIdentifier::new_sha256_with_rsa_encryption(),
                issuer,
                validity,
                subject,
                subject_public_key_info,
                extensions: extensions.into(),
            },
            signature_algorithm: AlgorithmIdentifier::new_sha256_with_rsa_encryption(),
            signature_value: BitString::with_bytes(&pkcs7[1082..1594]).into(),
        };
        check_serde!(full_certificate: Certificate in pkcs7[45..1594]);

        let full_certificate = picky_asn1_der::from_bytes(&picky_asn1_der::to_vec(&full_certificate).unwrap()).unwrap();
        let signed_data = SignedData {
            version: CmsVersion::V1,
            digest_algorithms: DigestAlgorithmIdentifiers(Vec::new().into()),
            content_info,
            certificates: CertificateSet(vec![CertificateChoices::Certificate(full_certificate)]).into(),
            crls: Some(RevocationInfoChoices(Vec::new())),
            signers_infos: SignersInfos(Vec::new().into()),
        };

        check_serde!(signed_data: SignedData in pkcs7[19..1598]);
    }

    #[test]
    fn decode_with_crl() {
        let decoded = base64::decode(
            "MIIIxwYJKoZIhvcNAQcCoIIIuDCCCLQCAQExADALBgkqhkiG9w0BBwGgggXJMIIF\
                xTCCA62gAwIBAgIUFYedpm34R9SrNONqEn43NrNlDHMwDQYJKoZIhvcNAQELBQAw\
                cjELMAkGA1UEBhMCZmYxCzAJBgNVBAgMAmZmMQswCQYDVQQHDAJmZjELMAkGA1UE\
                CgwCZmYxCzAJBgNVBAsMAmZmMQ8wDQYDVQQDDAZDQU5hbWUxHjAcBgkqhkiG9w0B\
                CQEWD2NhbWFpbEBtYWlsLmNvbTAeFw0yMTA0MTkxNTQxNDlaFw0yNjA0MTkxNTQx\
                NDlaMHIxCzAJBgNVBAYTAmZmMQswCQYDVQQIDAJmZjELMAkGA1UEBwwCZmYxCzAJ\
                BgNVBAoMAmZmMQswCQYDVQQLDAJmZjEPMA0GA1UEAwwGQ0FOYW1lMR4wHAYJKoZI\
                hvcNAQkBFg9jYW1haWxAbWFpbC5jb20wggIiMA0GCSqGSIb3DQEBAQUAA4ICDwAw\
                ggIKAoICAQCwVfg08dBZyObLkyZufYCZ396B17ICMAjYUWjk2pfK3Q/3C0vCjppd\
                F5VW0g49D/ULV7tzRc3AZecw9RxHuwkeXioIZ6NQ92qdg8CnkOPLrSyDlMyDZgYU\
                NSFdpz81Bu0v17sUHfREz41Wi5CvdK9qSS/IiuZhEpKYx1trGAc22YwXLBGs6Dcb\
                jf3C8zRnG1FCsOYukaG6wUdzUtwkrgOIIMERTqZ1U5s0rXehg4Kb3chAsA31xvKT\
                UhMNfovjI+5FDB/ZjZOOPMobnN6E7DLFjBzpa11eFywPFvimNxWjN26HkEceIh7y\
                Hm/9GrlSvpXnZQRFNNKIIQBkHt6jbpByxIhU9Yq0uWSZNWk+c34H6sksWZtJpVvM\
                YWIGziatkr2Rjskn9xjSNFNHacj5u3j2KKGxCtkxrCXiLY9Chf1CfbhmLpdECTPW\
                fgOOzXu/GIFXaxsh0+NqodEChaA5GDztweqt7Ep3/V9c/ITWONzj8SOj97R5OYy8\
                rtu24YY+ft2PkRYRSwsJzHs4KfDaf1yN0WCBZSl1itVW7qsEKQ60pp4qOna8XbyN\
                6VY3ce/qhKYPZKs9pFWX5vBTtAFcA4HjmT/EkHJ2ISJU0ueU0E6iH9Q01ENk1dso\
                dDMKP354kqmuHW4I0Wc39tJsXdUsGaisVyfOZdJQpqc2sle6LR8WpQIDAQABo1Mw\
                UTAdBgNVHQ4EFgQUxy69gEp87JxLsLZKmjYUObTIei4wHwYDVR0jBBgwFoAUxy69\
                gEp87JxLsLZKmjYUObTIei4wDwYDVR0TAQH/BAUwAwEB/zANBgkqhkiG9w0BAQsF\
                AAOCAgEAZzl2JCK3+5lm5vkcidD4swafCkE+kKuS27Ta7RVjfvEmZQmE69meMxuS\
                PZ/xcB2EVF+zTMw5qmtOiCFrN90sR5lGHhdkqFDMAhQtu7aGz9rJ8s9R4Hx8NWXk\
                d9H6USqWd6O3OIVl4cB7HZEdn4Y+oVgtSd48EAK4C/a2a9NLsrpPgGAB2m3wGRBY\
                l9ynvaR9hMXc0qQ/s/BseA/UvEaVVd1mxKcTeL4dGQpBJ9bKYbesJGHJssRnSlDb\
                AMI5tgoIYPadumNEqmDZL+eH798lfnyXCLokSZk/I7a4TLgtTnlubbFnapu8M645\
                qOS2VWzKnqRYC31DDWy5PcI/eDfLNCzrben9I6nSAx5AFOfi8z7znAwXw1fGcUEw\
                BPSzK91KHZkqsOaK/kExja32xeSSy7SW1dBHmwaDbA0kv7COPYCHIWmFsrxFkB9E\
                O5P1hSnFMZmdAO2jm/k0cZQxlaYZuio0XCQEJZMvfsGL8qWV5uRdUx8D5zZQem/R\
                OHEe1tMTIqJ3BoGgX15atokFY++iVLjk/2eKv1k5Sw5m/4cxxDgcK89UH4Y1UR3u\
                ah3emGU6zySj/Y3HpFfKslewb59FZXS/RKgRHhIw1TfauuTNtT5D2LpXPYfLuTrs\
                aCpH/QGsSBGiMTmrdXukRCIsz663TKiLVYOdvY4Y+cBcJlk/YMChggLPMIICyzCB\
                tAIBATANBgkqhkiG9w0BAQUFADByMQswCQYDVQQGEwJmZjELMAkGA1UECAwCZmYx\
                CzAJBgNVBAcMAmZmMQswCQYDVQQKDAJmZjELMAkGA1UECwwCZmYxDzANBgNVBAMM\
                BkNBTmFtZTEeMBwGCSqGSIb3DQEJARYPY2FtYWlsQG1haWwuY29tFw0yMTA0MjAw\
                NjUyMjRaFw0yMzA0MjAwNjUyMjRaoA4wDDAKBgNVHRQEAwIBAzANBgkqhkiG9w0B\
                AQUFAAOCAgEAW/+H6pzGp6cRUssck1a5pAJo2V98gpLX5gnPoorEIE2nkcLChiWc\
                RCdJuc5PtOisM/FRl9IxQWpePISB3I15jaL1u1ol5ISNn69f3eWwvVEw3kJSEeb/\
                TYvqW0+k1CgMr84oP38K4/434FwfotULX36FdU04MSzMirAszjZ0kLMsb3mNSSaH\
                VC0kZs7AnvzwKBXsB143ouNAH5mmLom1EyRAWU1ZP/pFZXDGE1ct2jB+oOdZebQj\
                /4VjfGDyvzUd9cNu9i6ZqNf49E9vhemrCdkZHc94QkwO92FhBROZhQ9fKelV8CRs\
                jf2oyToe+2NN2eXj+DY/s13Knoeqb7FcD3BFObtrILvE/rrCxZa0JeHfdg79nIiG\
                BCfQloA+cZdQsCQ1H1Qd3kwqo6ZLQpeTyW0UeIJNLQiSMATvpMAtunwT/OgxSP/Q\
                eTXV+221Eu2tDhXYMVkFtjgFdp0O5XqPU5fNPF/5XL3DlgAaWe9ULl4ZwBNPSkOm\
                LiFMcN1hzGQQo00ycuU6eF+Iz+H/olJyrpdJxf0jh2Sok71LX6YlALvfvZjW5eYc\
                8AvDttigOLiDwm8eYAxsC8Ku4cMiMSkgs71vvmz0U/LHypZiNJsEEaR76NH9OLiz\
                XCIYfP7WudYgfGBRRiw4WeB7jZNtVzFzkyiwliZLqocBuM8f1O2pv/QxAA==",
        )
        .unwrap();

        let mut issuer = Name::new();
        issuer.add_attr(NameAttr::CountryName, PrintableString::new("ff").unwrap());
        issuer.add_attr(NameAttr::StateOrProvinceName, "ff");
        issuer.add_attr(NameAttr::LocalityName, "ff");
        issuer.add_attr(NameAttr::OrganizationName, "ff");
        issuer.add_attr(NameAttr::OrganizationalUnitName, "ff");
        issuer.add_attr(NameAttr::CommonName, "CAName");
        issuer.add_email(IA5String::new("camail@mail.com").unwrap());

        let tbs_cert_list = TbsCertList {
            version: Some(Version::V2),
            signature: AlgorithmIdentifier::new_sha1_with_rsa_encryption(),
            issuer,
            this_update: UTCTime::new(2021, 4, 20, 6, 52, 24).unwrap().into(),
            next_update: Some(UTCTime::new(2023, 4, 20, 6, 52, 24).unwrap().into()),
            revoked_certificates: None,
            crl_extension: Some(Extensions(vec![Extension::new_crl_number(OctetStringAsn1Container(
                IntegerAsn1::from(decoded[1716..1717].to_vec()),
            ))]))
            .into(),
        };

        check_serde!(tbs_cert_list: TbsCertList in decoded[1534..1717]);

        let crl = RevocationInfoChoices(vec![RevocationInfoChoice::Crl(CertificateList {
            tbs_cert_list,
            signature_algorithm: AlgorithmIdentifier::new_sha1_with_rsa_encryption(),
            signature_value: BitString::with_bytes(&decoded[1737..2249]).into(),
        })]);

        check_serde!(crl: RevocationInfoChoices in decoded[1526..2249]);
    }

    #[test]
    fn decode_certificate_trust_list_certificate_set() {
        let decoded = base64::decode(
            "\
        oIINKTCCBhQwggP8oAMCAQICEzMAAABWo7N5AjhScwQAAAAAAFYwDQYJKoZIhvcN\
        AQELBQAwgYExCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYD\
        VQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKzAp\
        BgNVBAMTIk1pY3Jvc29mdCBDZXJ0aWZpY2F0ZSBMaXN0IENBIDIwMTEwHhcNMjAx\
        MjE1MjEyNTI0WhcNMjExMjAyMjEyNTI0WjCBiTELMAkGA1UEBhMCVVMxEzARBgNV\
        BAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQxHjAcBgNVBAoTFU1pY3Jv\
        c29mdCBDb3Jwb3JhdGlvbjEzMDEGA1UEAxMqTWljcm9zb2Z0IENlcnRpZmljYXRl\
        IFRydXN0IExpc3QgUHVibGlzaGVyMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIB\
        CgKCAQEAwXbtTwWpw7LVTZ3PWvV+xvhv7FZALwvCKjBVWINWi02HmMvxlgJM7y4Z\
        EOfG4A6PUyXBn7rSSLx1zz309yRYvBjkkY7Ai7S+eG8z99P5AJmVXAkm9AJccFEb\
        e5jw3HyLAYTEJiR9X550pYb3w29HLrCz9hGDZ+PJ2nGiXGQFnNkrJuY9yYBoEYRU\
        CFWqs1HvX5GcfDT0MuJZpMlg5bG4rvm5+t6Ge5aq9qlgD5YAxFLrEQbZ89BBHRG4\
        PODqrYm4+CYWmZADIBxc9aPC5ZWAGYjBc3vu7iMAZu8IDpSeOre2EChCzHiJZn/b\
        nguWA6sUvJ3QKx0Gsyld4xbLWhfWGwIDAQABo4IBeTCCAXUwFQYDVR0lBA4wDAYK\
        KwYBBAGCNwoDCTAdBgNVHQ4EFgQUIIpJhsQJpHIBXXfSdyHTm19nzx4wVAYDVR0R\
        BE0wS6RJMEcxLTArBgNVBAsTJE1pY3Jvc29mdCBJcmVsYW5kIE9wZXJhdGlvbnMg\
        TGltaXRlZDEWMBQGA1UEBRMNMjI5ODg3KzQ2Mjk5MjAfBgNVHSMEGDAWgBRB8CHH\
        7cSH+oN1/woM3C3sqGqrWTBZBgNVHR8EUjBQME6gTKBKhkhodHRwOi8vY3JsLm1p\
        Y3Jvc29mdC5jb20vcGtpL2NybC9wcm9kdWN0cy9NaWNDZXJMaXNDQTIwMTFfMjAx\
        MS0wMy0yOS5jcmwwXQYIKwYBBQUHAQEEUTBPME0GCCsGAQUFBzAChkFodHRwOi8v\
        d3d3Lm1pY3Jvc29mdC5jb20vcGtpL2NlcnRzL01pY0Nlckxpc0NBMjAxMV8yMDEx\
        LTAzLTI5LmNydDAMBgNVHRMBAf8EAjAAMA0GCSqGSIb3DQEBCwUAA4ICAQB36571\
        4rZo6lGtH4lxhxjjgzdsFuJs3vQwuIMulUuaF0+viu4966O+SqX3PtRLj97CqgI5\
        wLZK5Ib03ytIFlZ35Q1AE63yPl5gU8LDF2KkE+/kuWkHhxCNMXbQWfsH/7mIbzbo\
        PXoixiMHwBsWmEg/Nmk2Ya23NBdnKeGxEv7EI81kejbePacEMzIeXha4vLFrWzsT\
        FXICjVh47GcHSCwcGRp2G/wkItekTpXMrkdWr1cjaXHqxqlPorfr7zAoBBkJWMKC\
        Wfo09voRYXEhp4TE4ZkMzS+Q4GWyOxU0hCBaQPEt4lm5x5exJPkByGfKVZRhzr4z\
        9IRIptO0ozTSjs9nl7eg5dDqgf/MfoZyTY4mhuGsJqGbwxIoBC/kTbvQP35zjMeT\
        66w8pxx/E+qDunzBWZkKXS4kdgpnb4Mpr3gAIeHpPb+ijiqhB0mS1gKvbCAx4OIZ\
        YHJcZU92trWagrRLzS0rvd2WVC/3kkFvcSt5AQg2cJkeKEUOHKGtH6gUzxd1GE11\
        XhQO+GTMihVqApJ1KFxrjtZ5J2ZZVM+bd908OAfCEpG5+fFi2FhJZ7LKydWzCGbH\
        P7YASXdZ94lGtBqGm8a5FiQAwTOuUaIaHXql8IQAVAqyUpKEDBjl1BcKvb7drWHV\
        HeNYLDMntpdv+KAX/WtLapSBsrbxSFlCE3Ag8TCCBw0wggT1oAMCAQICCmERbJIA\
        AAAAAAcwDQYJKoZIhvcNAQELBQAwgYgxCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpX\
        YXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQg\
        Q29ycG9yYXRpb24xMjAwBgNVBAMTKU1pY3Jvc29mdCBSb290IENlcnRpZmljYXRl\
        IEF1dGhvcml0eSAyMDEwMB4XDTExMDMyOTE4NTgzOVoXDTI2MDMyOTE5MDgzOVow\
        gYExCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdS\
        ZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKzApBgNVBAMT\
        Ik1pY3Jvc29mdCBDZXJ0aWZpY2F0ZSBMaXN0IENBIDIwMTEwggIiMA0GCSqGSIb3\
        DQEBAQUAA4ICDwAwggIKAoICAQC4hHqA/U0np9LvgjbxUwWdvkJtjjEIR8vNs4MK\
        S04zGi583XKBjc6Q/DwFyy80hTiPPBxobWBRUk2s2m0rfrNzR6vS3JVxSjGBWqEf\
        q4ImRS2M6IR4vSDwDcb1riaHHlaOVTwIMDKUlCKTC6Wwxl3mLYE5zenHrujYSXFJ\
        q5F0uE+NL0ezP9CTg1wCGt5LuLI8N+mT6nJbmMfjrBjg5n5KwYEs/SIUdnPhaNwg\
        CcDzRs0jJshFIsrHvHT8if9X4M+9jrAr7ybWd6sa9GdB8V4McaoCf17AgqoJi+yJ\
        iEH1A0Jp2R9F2Vc+BJZK1TK30WEmaMfBsaDgegVOtW3CguAutudknxZ9lSqGMtAh\
        yF34ywUwHrkCmGyzk2vIg2chXdZlmCBk3cu/R5v/GPrxkN6neM17BIZ+J4q3lZwm\
        3bGW/E/gQCCDaN3sM/IqoAen65H6rA9RQYjxxYdBTIdHYp1YwJ5/uxJ93tOf/cHH\
        FL1/mNBXm+HjbFfhZV/w3CucoVTCVioVZMuqTuT9w+h3iP/bDa+Qn9dogQEvlOGv\
        xuTGdtt12t/QEkzyiTZvSICBWN0XCSgrVayTI+WOMWWtDY6T03GngRSY6ayqBVju\
        10RDMG0dx7rCf/VIxOWgjlWOtAnAAcOdHUb1/ka1OgCII7XwykHNOw3G9spABOqb\
        5Yg2nwIDAQABo4IBfDCCAXgwEAYJKwYBBAGCNxUBBAMCAQAwHQYDVR0OBBYEFEHw\
        IcftxIf6g3X/CgzcLeyoaqtZMBkGCSsGAQQBgjcUAgQMHgoAUwB1AGIAQwBBMAsG\
        A1UdDwQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFNX2VsuP6KJc\
        YmjRPZSQW9fOmhjEMFYGA1UdHwRPME0wS6BJoEeGRWh0dHA6Ly9jcmwubWljcm9z\
        b2Z0LmNvbS9wa2kvY3JsL3Byb2R1Y3RzL01pY1Jvb0NlckF1dF8yMDEwLTA2LTIz\
        LmNybDBaBggrBgEFBQcBAQROMEwwSgYIKwYBBQUHMAKGPmh0dHA6Ly93d3cubWlj\
        cm9zb2Z0LmNvbS9wa2kvY2VydHMvTWljUm9vQ2VyQXV0XzIwMTAtMDYtMjMuY3J0\
        MDcGA1UdJQQwMC4GCCsGAQUFBwMDBgorBgEEAYI3CgMBBgorBgEEAYI3CgMJBgor\
        BgEEAYI3CgMTMA0GCSqGSIb3DQEBCwUAA4ICAQCC96mls7/lyFlBJzQPYpxB8Ksr\
        ffmnqMioD11Dvq3ymfj/+/Z5UEQMUOpC250B6aVJeSgpEz5ANnQW248gzI0tURDc\
        K0E2fLbQQBObHAA3TIFpaLEagpY7aXXH5TTYPtxaCavTv6mvxAhv40fGMu8l6QsL\
        VRKU74cUGdLhId43z66mNF0jKQQEbW3nGt1EMHl0Go2Mwfk++a0wa6O0anRJOVs3\
        LQEZ7gMp3a5KL/mCr0gfFJqcPSUxVuo6p023/Ys//r93NotV5bMQUO5U1L9r2Cry\
        M3guvzH5NhHvMAv5TEOD41upXB1bpnYFuPB1T+m4HzZEpn9m0EsNGFUedC4nJ+Um\
        QoNuu6Tve/nkmL3VO4nTWJK40c0Wfzl+ZiUN24NZv1cfm9LpG3InXWsz0f6ikkxR\
        PcbMlDpW/+sQQS7dklPNEPEdNusEGts12ZG2mWAP4AusZwxEFpwCR8u3RpZJD98D\
        sQ+tDhKtSAU24S0/u1rglNSXkuk+5uslGcsz8d+TYJCmuQ5W9ijpQsceEFumLg+5\
        26lk147lM9KdQ4JLbjdmuQ1nV1RaSA7jivsf7QomvA000gpHhWEqI7HgVIpQFFaF\
        wP8t92mZRH0a9E18GA7hBwfuCWZSSnoaYqTli8+FooaKcZCxfdYR01Ee2lznzNYS\
        EHaork+TtWTJve3c+w==",
        )
        .unwrap();

        let certificate_set: CertificateSet = picky_asn1_der::from_bytes(&decoded).unwrap();
        check_serde!(certificate_set: CertificateSet in decoded);
    }
}
