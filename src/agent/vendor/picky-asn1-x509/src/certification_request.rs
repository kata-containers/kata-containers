use crate::attribute::{Attribute, Attributes};
use crate::{AlgorithmIdentifier, Name, SubjectPublicKeyInfo};
use picky_asn1::tag::Tag;
use picky_asn1::wrapper::{Asn1SequenceOf, BitStringAsn1};
use serde::{de, ser, Deserialize, Serialize};

/// [RFC 2986 #4](https://tools.ietf.org/html/rfc2986#section-4)
///
/// ```not_rust
/// CertificationRequestInfo ::= SEQUENCE {
///      version       INTEGER { v1(0) } (v1,...),
///      subject       Name,
///      subjectPKInfo SubjectPublicKeyInfo{{ PKInfoAlgorithms }},
///      attributes    [0] Attributes{{ CRIAttributes }}
/// }
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CertificationRequestInfo {
    pub version: u8,
    pub subject: Name,
    pub subject_public_key_info: SubjectPublicKeyInfo,
    pub attributes: OpenSslAttributes,
}

/// OpenSSL compatible CRI `attributes` type
///
/// For some reason, openssl is producing an "implicit context tag" but with coding `0xA0`
///
/// In [appendix A of the RFC](https://datatracker.ietf.org/doc/html/rfc2986#appendix-A)
/// ```not_rust
/// DEFINITIONS IMPLICIT TAGS ::=
/// ```
/// states that context tags are implicit unless stated otherwise.
/// This type implements a workaround to imitate this.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenSslAttributes(pub Attributes);

impl std::ops::Deref for OpenSslAttributes {
    type Target = Attributes;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for OpenSslAttributes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ser::Serialize for OpenSslAttributes {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        let mut raw_der = picky_asn1_der::to_vec(&self.0).unwrap();
        raw_der[0] = Tag::context_specific_constructed(0).inner();
        picky_asn1_der::Asn1RawDer(raw_der).serialize(serializer)
    }
}

impl<'de> de::Deserialize<'de> for OpenSslAttributes {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        let mut raw_der = picky_asn1_der::Asn1RawDer::deserialize(deserializer)?.0;
        raw_der[0] = Tag::SEQUENCE.inner();
        let attrs = picky_asn1_der::from_bytes(&raw_der).unwrap_or_default();
        Ok(Self(attrs))
    }
}

impl CertificationRequestInfo {
    pub fn new(subject: Name, subject_public_key_info: SubjectPublicKeyInfo) -> Self {
        // It shall be 0 for this version of the standard.
        Self {
            version: 0,
            subject,
            subject_public_key_info,
            attributes: OpenSslAttributes(Asn1SequenceOf(Vec::new())),
        }
    }

    pub fn with_attribute(mut self, attribute: Attribute) -> Self {
        self.attributes.0.push(attribute);
        self
    }

    pub fn add_attribute(&mut self, attribute: Attribute) {
        self.attributes.0.push(attribute);
    }
}

/// [RFC 2986 #4](https://tools.ietf.org/html/rfc2986#section-4)
///
/// ```not_rust
/// CertificationRequest ::= SEQUENCE {
///      certificationRequestInfo CertificationRequestInfo,
///      signatureAlgorithm AlgorithmIdentifier{{ SignatureAlgorithms }},
///      signature          BIT STRING
/// }
/// ```
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CertificationRequest {
    pub certification_request_info: CertificationRequestInfo,
    pub signature_algorithm: AlgorithmIdentifier,
    pub signature: BitStringAsn1,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name::*;
    use crate::{DirectoryName, Extension, GeneralName};
    use picky_asn1::bit_string::BitString;
    use picky_asn1::restricted_string::{IA5String, PrintableString, Utf8String};
    use picky_asn1::wrapper::IntegerAsn1;
    use std::str::FromStr;

    #[test]
    fn basic_csr() {
        let encoded = base64::decode(
            "MIICYjCCAUoCAQAwHTEbMBkGA1UEAxMSdGVzdC5jb250b3NvLmxvY2FsMIIBIjAN\
            BgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAym0At2TvEqP0mYVLJzGVpNXjugu/\
            kBpuKvXt/Vax4Bxnj3YzHTCpwkyZPytUC6zJ+q+uGh0e7gYQsYHJKjgoKEsS6gQ4\
            ZM3D/AQy0zqPUT0ruSKDWKK4f2d/2ijDs5R2LHj7DtNZBanCXU16Qp1O28su0QZK\
            OYbXzsJSpHp80dhqD6JUxXlSZzlVBp28CC9ryrE6w+kOQ38TZ1/mBJPsfmDeKBpm\
            3FRrfHtWt43eok/T6FhCLIzsqyCZ0UCQqkcLr+TfoftJe2nOHQ1sfk4keJ9iwA/f\
            hYv5rqUB3RUztSIhExwtYDwd+YovenhsL4sW/kjR29RTLUFPPXAelG9XPwIDAQAB\
            oAAwDQYJKoZIhvcNAQELBQADggEBAKrCf4sFDBFZQ6CPYdaxe3InMp7KFaueMIB8\
            /YK73rJ+JGB6fQfltCCkToTE1y0Q3UqTlqHmaqdoh0KMWue6jCFvBat4/TUqUG7W\
            tRLDP67eMulolcIzLqwTjR38DVJvnwrd2pey43q3UHBjlStxT/gI4ysQHn4qrzHB\
            6OK9O6ypqTtwXxnm3TJF9dctLwvbh7NZSaamSlxI0/ajKZOP9k1KZEOPtaiiMPe2\
            yr+QvwY2ov66MRG5PPRZELQWBaPZOuFwmCsFOLXJMpvhoAgklBCFZmiQMgApGIC1\
            FIDgjm2ZhQQIRMnTsAV6f7BclRTaUkc0sPl17YB9GfNfOm1oL7o=",
        )
        .expect("invalid base64");

        let certification_request_info = CertificationRequestInfo::new(
            DirectoryName::new_common_name(PrintableString::from_str("test.contoso.local").unwrap()).into(),
            SubjectPublicKeyInfo::new_rsa_key(
                IntegerAsn1::from(encoded[74..331].to_vec()),
                IntegerAsn1::from(encoded[333..336].to_vec()),
            ),
        );

        check_serde!(certification_request_info: CertificationRequestInfo in encoded[4..338]);

        let csr = CertificationRequest {
            certification_request_info,
            signature_algorithm: AlgorithmIdentifier::new_sha256_with_rsa_encryption(),
            signature: BitString::with_bytes(&encoded[358..614]).into(),
        };

        check_serde!(csr: CertificationRequest in encoded);
    }

    #[test]
    fn csr_with_extensions_attribute() {
        let encoded = base64::decode(
            "MIICjDCCAXQCAQAwIDELMAkGA1UEBhMCWFgxETAPBgNVBAMMCHNvbWV0ZXN0MIIB\
            IjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAvNELh212N4optYS7pqbtvjyv\
            +t4fQjX/pwB88BUCEjBgh+DJ49EBPQg9oObADTcBi3EeXu4M5y6f/dzIhovayJ/y\
            9j7Cj0Bw+VY+eRXywkVG/DqaiKG2mIQW+fho7/jhazhpeIxCzObPTwiQK7i96Vjq\
            9S+o4QQejE2SYLOhQ4/cgUaT7JBm4yab7cvhFjKYjVmoP6ioIcHb9Cmv25Lttuvk\
            n64bDiPKz6BkutRpbMipQjSA8xKEgjgFG/nxBynA8PXnZIunhTNyhXrqRoAe6SXn\
            ZLZLmwOkeU5WTewVVTXlmqaZTPwtb/9EjjoRnO3+Ulb5zT5wPULc79xuY16kzwID\
            AQABoCcwJQYJKoZIhvcNAQkOMRgwFjAUBgNVHREEDTALgglsb2NhbGhvc3QwDQYJ\
            KoZIhvcNAQELBQADggEBAIm9lOhZG3XY4CNJ5b18Qu/OfFi+T0tgxt4bTqINQ1Iz\
            SQFrsnheBrzmasfFliz10N96cOmNka1UpWqK7N5/TfkJHX3zKYRpc2jEkrFun48B\
            3+bOJJPH48zmTGxBgU7iiorpaVt3CpgXNswhU3fpcT5gLy8Ys7DXC39Nn1lW0Lko\
            cd6xK4oIJyoeiXyVBdn68gtPY6xjFxta67nyj39sSGhATxrDgxtLHEH2+HStywr0\
            4/osg9vP/OH5iFYOiEimK6ErYNg8rM1A/OTe5p8emA6y3o5dHG8lKYwevyUXMSLv\
            38CNeh0MS2KmyHz2085HlIIAXIu2xAUyWLsQik+eV6M=",
        )
        .expect("invalid base64");

        let extensions = vec![Extension::new_subject_alt_name(vec![GeneralName::DnsName(
            IA5String::from_string("localhost".into()).unwrap().into(),
        )])
        .into_non_critical()];

        let mut dn = DirectoryName::new();
        dn.add_attr(NameAttr::CountryName, PrintableString::from_str("XX").unwrap());
        dn.add_attr(NameAttr::CommonName, Utf8String::from_str("sometest").unwrap());

        let certification_request_info = CertificationRequestInfo::new(
            dn.into(),
            SubjectPublicKeyInfo::new_rsa_key(
                IntegerAsn1::from(encoded[77..334].to_vec()),
                IntegerAsn1::from(encoded[336..339].to_vec()),
            ),
        )
        .with_attribute(Attribute::new_extension_request(extensions));

        check_serde!(certification_request_info: CertificationRequestInfo in encoded[4..380]);

        let csr = CertificationRequest {
            certification_request_info,
            signature_algorithm: AlgorithmIdentifier::new_sha256_with_rsa_encryption(),
            signature: BitString::with_bytes(&encoded[400..656]).into(),
        };

        check_serde!(csr: CertificationRequest in encoded);
    }
}
