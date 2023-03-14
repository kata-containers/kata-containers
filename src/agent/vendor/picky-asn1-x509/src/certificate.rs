use crate::{
    AlgorithmIdentifier, AuthorityKeyIdentifier, BasicConstraints, Extension, ExtensionView, Extensions, Name,
    SubjectPublicKeyInfo, Validity, Version,
};
use picky_asn1::wrapper::{BitStringAsn1, ExplicitContextTag0, ExplicitContextTag3, IntegerAsn1};
use serde::{de, Deserialize, Serialize};
use std::fmt;

/// [RFC 5280 #4.1](https://tools.ietf.org/html/rfc5280#section-4.1)
///
/// ```not_rust
/// Certificate  ::=  SEQUENCE  {
///      tbsCertificate       TBSCertificate,
///      signatureAlgorithm   AlgorithmIdentifier,
///      signatureValue       BIT STRING  }
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Certificate {
    pub tbs_certificate: TbsCertificate,
    pub signature_algorithm: AlgorithmIdentifier,
    pub signature_value: BitStringAsn1,
}

impl Certificate {
    fn h_find_extension(&self, key_identifier_oid: &oid::ObjectIdentifier) -> Option<&Extension> {
        (self.tbs_certificate.extensions.0)
            .0
            .iter()
            .find(|ext| ext.extn_id() == key_identifier_oid)
    }

    pub fn subject_key_identifier(&self) -> Option<&[u8]> {
        let ext = self.h_find_extension(&crate::oids::subject_key_identifier())?;
        match ext.extn_value() {
            ExtensionView::SubjectKeyIdentifier(ski) => Some(&ski.0),
            _ => None,
        }
    }

    pub fn authority_key_identifier(&self) -> Option<&AuthorityKeyIdentifier> {
        let ext = self.h_find_extension(&crate::oids::authority_key_identifier())?;
        match ext.extn_value() {
            ExtensionView::AuthorityKeyIdentifier(aki) => Some(aki),
            _ => None,
        }
    }

    pub fn basic_constraints(&self) -> Option<&BasicConstraints> {
        let ext = self.h_find_extension(&crate::oids::basic_constraints())?;
        match ext.extn_value() {
            ExtensionView::BasicConstraints(bc) => Some(bc),
            _ => None,
        }
    }

    pub fn extensions(&self) -> &[Extension] {
        (self.tbs_certificate.extensions.0).0.as_slice()
    }
}

/// [RFC 5280 #4.1](https://tools.ietf.org/html/rfc5280#section-4.1)
///
/// ```not_rust
/// TBSCertificate  ::=  SEQUENCE  {
///      version         [0]  EXPLICIT Version DEFAULT v1,
///      serialNumber         CertificateSerialNumber,
///      signature            AlgorithmIdentifier,
///      issuer               Name,
///      validity             Validity,
///      subject              Name,
///      subjectPublicKeyInfo SubjectPublicKeyInfo,
///      issuerUniqueID  [1]  IMPLICIT UniqueIdentifier OPTIONAL,
///                           -- If present, version MUST be v2 or v3
///      subjectUniqueID [2]  IMPLICIT UniqueIdentifier OPTIONAL,
///                           -- If present, version MUST be v2 or v3
///      extensions      [3]  EXPLICIT Extensions OPTIONAL
///                           -- If present, version MUST be v3
///      }
/// ```
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TbsCertificate {
    #[serde(skip_serializing_if = "version_is_default")]
    pub version: ExplicitContextTag0<Version>,
    pub serial_number: IntegerAsn1,
    pub signature: AlgorithmIdentifier,
    pub issuer: Name,
    pub validity: Validity,
    pub subject: Name,
    pub subject_public_key_info: SubjectPublicKeyInfo,
    // issuer_unique_id
    // subject_unique_id
    #[serde(skip_serializing_if = "extensions_are_empty")]
    pub extensions: ExplicitContextTag3<Extensions>,
}

fn version_is_default(version: &Version) -> bool {
    version == &Version::default()
}

// Implement Deserialize manually to support missing version field (i.e.: fallback as V1)
impl<'de> de::Deserialize<'de> for TbsCertificate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = TbsCertificate;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct TBSCertificate")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let version: ExplicitContextTag0<Version> = seq.next_element().unwrap_or_default().unwrap_or_default();
                let serial_number = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let signature = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let issuer = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let validity = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let subject = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(5, &self))?;
                let subject_public_key_info = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(6, &self))?;
                let extensions: ExplicitContextTag3<Extensions> = seq
                    .next_element()?
                    .unwrap_or_else(|| Some(Extensions(Vec::new()).into()))
                    .unwrap_or_else(|| Extensions(Vec::new()).into());

                if version.0 != Version::V3 && !(extensions.0).0.is_empty() {
                    return Err(serde_invalid_value!(
                        TbsCertificate,
                        "Version is not V3, but Extensions are present",
                        "no Extensions"
                    ));
                }

                Ok(TbsCertificate {
                    version,
                    serial_number,
                    signature,
                    issuer,
                    validity,
                    subject,
                    subject_public_key_info,
                    extensions,
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

fn extensions_are_empty(extensions: &Extensions) -> bool {
    extensions.0.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DirectoryName, Extension, KeyIdentifier, KeyUsage};
    use num_bigint_dig::BigInt;
    use picky_asn1::bit_string::BitString;
    use picky_asn1::date::UTCTime;

    #[test]
    fn x509_v3_certificate() {
        let encoded = base64::decode(
            "MIIEGjCCAgKgAwIBAgIEN8NXxDANBgkqhkiG9w0BAQsFADAiMSAwHgYDVQQ\
             DDBdjb250b3NvLmxvY2FsIEF1dGhvcml0eTAeFw0xOTEwMTcxNzQxMjhaFw0yMjEwM\
             TYxNzQxMjhaMB0xGzAZBgNVBAMMEnRlc3QuY29udG9zby5sb2NhbDCCASIwDQYJKoZ\
             IhvcNAQEBBQADggEPADCCAQoCggEBAMptALdk7xKj9JmFSycxlaTV47oLv5Aabir17\
             f1WseAcZ492Mx0wqcJMmT8rVAusyfqvrhodHu4GELGBySo4KChLEuoEOGTNw/wEMtM\
             6j1E9K7kig1iiuH9nf9oow7OUdix4+w7TWQWpwl1NekKdTtvLLtEGSjmG187CUqR6f\
             NHYag+iVMV5Umc5VQadvAgva8qxOsPpDkN/E2df5gST7H5g3igaZtxUa3x7VreN3qJ\
             P0+hYQiyM7KsgmdFAkKpHC6/k36H7SXtpzh0NbH5OJHifYsAP34WL+a6lAd0VM7UiI\
             RMcLWA8HfmKL3p4bC+LFv5I0dvUUy1BTz1wHpRvVz8CAwEAAaNdMFswCQYDVR0TBAI\
             wADAOBgNVHQ8BAf8EBAMCAaAwHQYDVR0OBBYEFCMimIgHf5c00sI9jZzeWoMLsR60M\
             B8GA1UdIwQYMBaAFBbHC24DEnsUFLz/zmqB5cMCHo9OMA0GCSqGSIb3DQEBCwUAA4I\
             CAQA1ehZTTBbes2DgGXwQugoV9PdOGMFEVT4dzrrluo/4exSfqLrNuY2NXVuNBKW4n\
             DA5aD71Q/KUZ8Y8cV9qa8OBJQvQ0dd0qeHmeEYdDsj5YD4ECycKx9U1ZX5fi6tpSIX\
             6DsietpCnrw4aTgbEOvMeQcuYCTP30Vpt+mYEKBlR/E2Vcl2zUD+67gqppSaC1RceL\
             /8Cy6ZXlPqwmS2zqK9UhYVRKlEww8xSh/9CR9MmIDc4pHtCpMawcn6Dmo+A+LcKi5v\
             /NIwvSJTei+h1gvRhvEOPcf4VZJMHXquNrxkMsKpuu7g/AYH7wl2MBaNaxyNlXY5e5\
             OjxslrbRCfDab11YaJEONcBnapl/+Ajr70uVFN09tDXyk0EHYf75NiRztgVKclna26\
             zP5qRb0JSYNQJW2kIIBX6DhU7kt6RcauF2hJ+jLWOF2vsAS8PdEr7vnR1EGOrrcQ3V\
             UgMscNsDqf50YMi2Inu1Kt2t+QSvYs61ON39aVpqR67nskdUWzFCVgWQVezM1ZagoO\
             yNp7WjRYl8hJ0YVZ7TRtP8nJOkZ6s046YHVWxMuGdqZfd/AUFb9xzzXjGRuuZ1JmSf\
             +VBOFEe2MaPMyMQBeIs3Othz6Fcy6Am5F6c3It31WYJwiCa/NdbMIvGy1xvAN5kzR/\
             Y6hkoQljoSr1rVuszJ9dtvuTccA==",
        )
        .expect("invalid base64");

        // Issuer

        let issuer: Name = DirectoryName::new_common_name("contoso.local Authority").into();
        check_serde!(issuer: Name in encoded[34..70]);

        // Validity

        let validity = Validity {
            not_before: UTCTime::new(2019, 10, 17, 17, 41, 28).unwrap().into(),
            not_after: UTCTime::new(2022, 10, 16, 17, 41, 28).unwrap().into(),
        };
        check_serde!(validity: Validity in encoded[70..102]);

        // Subject

        let subject: Name = DirectoryName::new_common_name("test.contoso.local").into();
        check_serde!(subject: Name in encoded[102..133]);

        // SubjectPublicKeyInfo

        let subject_public_key_info = SubjectPublicKeyInfo::new_rsa_key(
            IntegerAsn1::from(encoded[165..422].to_vec()),
            BigInt::from(65537).to_signed_bytes_be().into(),
        );
        check_serde!(subject_public_key_info: SubjectPublicKeyInfo in encoded[133..427]);

        // Extensions

        let mut key_usage = KeyUsage::new(7);
        key_usage.set_digital_signature(true);
        key_usage.set_key_encipherment(true);

        let extensions = Extensions(vec![
            Extension::new_basic_constraints(None, None).into_non_critical(),
            Extension::new_key_usage(key_usage),
            Extension::new_subject_key_identifier(&encoded[469..489]),
            Extension::new_authority_key_identifier(KeyIdentifier::from(encoded[502..522].to_vec()), None, None),
        ]);
        check_serde!(extensions: Extensions in encoded[429..522]);

        // SignatureAlgorithm

        let signature_algorithm = AlgorithmIdentifier::new_sha256_with_rsa_encryption();
        check_serde!(signature_algorithm: AlgorithmIdentifier in encoded[522..537]);

        // TbsCertificate

        let tbs_certificate = TbsCertificate {
            version: ExplicitContextTag0(Version::V3).into(),
            serial_number: BigInt::from(935548868).to_signed_bytes_be().into(),
            signature: signature_algorithm.clone(),
            issuer,
            validity,
            subject,
            subject_public_key_info,
            extensions: extensions.into(),
        };
        check_serde!(tbs_certificate: TbsCertificate in encoded[4..522]);

        // Full certificate

        let certificate = Certificate {
            tbs_certificate,
            signature_algorithm,
            signature_value: BitString::with_bytes(&encoded[542..1054]).into(),
        };
        check_serde!(certificate: Certificate in encoded);
    }

    #[test]
    fn key_id() {
        let encoded = base64::decode(
            "MIIDPzCCAiegAwIBAgIBATANBgkqhkiG9w0BAQUFADA7MQswCQYDVQQGEwJOTDER\
                MA8GA1UECgwIUG9sYXJTU0wxGTAXBgNVBAMMEFBvbGFyU1NMIFRlc3QgQ0EwHhcN\
                MTEwMjEyMTQ0NDA2WhcNMjEwMjEyMTQ0NDA2WjA8MQswCQYDVQQGEwJOTDERMA8G\
                A1UECgwIUG9sYXJTU0wxGjAYBgNVBAMMEVBvbGFyU1NMIFNlcnZlciAxMIIBIjAN\
                BgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAqQIfPUBq1VVTi/027oJlLhVhXom/\
                uOhFkNvuiBZS0/FDUEeWEllkh2v9K+BG+XO+3c+S4ZFb7Wagb4kpeUWA0INq1UFD\
                d185fAkER4KwVzlw7aPsFRkeqDMIR8EFQqn9TMO0390GH00QUUBncxMPQPhtgSVf\
                CrFTxjB+FTms+Vruf5KepgVb5xOXhbUjktnUJAbVCSWJdQfdphqPPwkZvq1lLGTr\
                lZvc/kFeF6babFtpzAK6FCwWJJxK3M3Q91Jnc/EtoCP9fvQxyi1wyokLBNsupk9w\
                bp7OvViJ4lNZnm5akmXiiD8MlBmj3eXonZUT7Snbq3AS3FrKaxerUoJUsQIDAQAB\
                o00wSzAJBgNVHRMEAjAAMB0GA1UdDgQWBBQfdNY/KcF0dEU7BRIsPai9Q1kCpjAf\
                BgNVHSMEGDAWgBS0WuSls97SUva51aaVD+s+vMf9/zANBgkqhkiG9w0BAQUFAAOC\
                AQEAm9GKWy4Z6eS483GoR5omwx32meCStm/vFuW+nozRwqwTG5d2Etx4TPnz73s8\
                fMtM1QB0QbfBDDHxfGymEsKwICmCkJszKE7c03j3mkddrrvN2eIYiL6358S3yHMj\
                iLVCraRUoEm01k7iytjxrcKb//hxFvHoxD1tdMqbuvjMlTS86kJSrkUMDw68UzfL\
                jvo3oVjiexfasjsICXFNoncjthKtS7v4zrsgXNPz92h58NgXnDtQU+Eb9tVA9kUs\
                Ln/az3v5DdgrNoAO60zK1zYAmekLil7pgba/jBLPeAQ2fZVgFxttKv33nUnUBzKA\
                Od8i323fM5dQS1qQpBjBc/5fPw==",
        )
        .expect("invalid base64");

        let cert: Certificate = picky_asn1_der::from_bytes(&encoded).expect("intermediate cert");

        pretty_assertions::assert_eq!(
            hex::encode(&cert.subject_key_identifier().unwrap()),
            "1f74d63f29c17474453b05122c3da8bd435902a6"
        );
        pretty_assertions::assert_eq!(
            hex::encode(&cert.authority_key_identifier().unwrap().key_identifier().unwrap()),
            "b45ae4a5b3ded252f6b9d5a6950feb3ebcc7fdff"
        );
    }
}
