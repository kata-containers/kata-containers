use crate::{oids, AlgorithmIdentifier, EcParameters};
use picky_asn1::wrapper::{BitStringAsn1, BitStringAsn1Container, IntegerAsn1, OctetStringAsn1};
use serde::{de, ser, Deserialize, Serialize};
use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub enum PublicKey {
    Rsa(EncapsulatedRsaPublicKey),
    Ec(EncapsulatedEcPoint),
    Ed(EncapsulatedEcPoint),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RsaPublicKey {
    pub modulus: IntegerAsn1,         // n
    pub public_exponent: IntegerAsn1, // e
}

pub type EncapsulatedRsaPublicKey = BitStringAsn1Container<RsaPublicKey>;

pub type EcPoint = OctetStringAsn1;

pub type EncapsulatedEcPoint = BitStringAsn1;

#[derive(Debug, PartialEq, Clone)]
pub struct SubjectPublicKeyInfo {
    pub algorithm: AlgorithmIdentifier,
    pub subject_public_key: PublicKey,
}

impl SubjectPublicKeyInfo {
    pub fn new_rsa_key(modulus: IntegerAsn1, public_exponent: IntegerAsn1) -> Self {
        Self {
            algorithm: AlgorithmIdentifier::new_rsa_encryption(),
            subject_public_key: PublicKey::Rsa(
                RsaPublicKey {
                    modulus,
                    public_exponent,
                }
                .into(),
            ),
        }
    }

    pub fn new_ec_key<P: Into<BitStringAsn1>>(ec_point: P) -> Self {
        Self {
            algorithm: AlgorithmIdentifier::new_elliptic_curve(EcParameters::NamedCurve(oids::ec_public_key().into())),
            subject_public_key: PublicKey::Ec(ec_point.into()),
        }
    }
}

impl ser::Serialize for SubjectPublicKeyInfo {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        use ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.algorithm)?;
        match &self.subject_public_key {
            PublicKey::Rsa(key) => seq.serialize_element(key)?,
            PublicKey::Ec(key) => seq.serialize_element(key)?,
            PublicKey::Ed(key) => seq.serialize_element(key)?,
        }
        seq.end()
    }
}

impl<'de> de::Deserialize<'de> for SubjectPublicKeyInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = SubjectPublicKeyInfo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded subject public key info")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let algorithm: AlgorithmIdentifier = seq_next_element!(seq, AlgorithmIdentifier, "algorithm oid");

                let subject_public_key = match Into::<String>::into(algorithm.oid()).as_str() {
                    oids::RSA_ENCRYPTION => PublicKey::Rsa(seq_next_element!(seq, SubjectPublicKeyInfo, "rsa key")),
                    oids::EC_PUBLIC_KEY => {
                        PublicKey::Ec(seq_next_element!(seq, SubjectPublicKeyInfo, "elliptic curves key"))
                    }
                    oids::ED25519 => PublicKey::Ed(seq_next_element!(seq, SubjectPublicKeyInfo, "curve25519 key")),
                    _ => {
                        return Err(serde_invalid_value!(
                            SubjectPublicKeyInfo,
                            "unsupported algorithm (unknown oid)",
                            "a supported algorithm"
                        ));
                    }
                };

                Ok(SubjectPublicKeyInfo {
                    algorithm,
                    subject_public_key,
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint_dig::BigInt;

    #[test]
    fn rsa_subject_public_key_info() {
        let encoded = base64::decode(
            "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAsiLoIx\
             mXaZAFRBKtHYZhiF8m+pYR+xGIpupvsdDEvKO92D6fIccgVLIW6p6sSNk\
             oXx5J6KDSMbA/chy5M6pRvJkaCXCI4zlCPMYvPhI8OxN3RYPfdQTLpgPy\
             wrlfdn2CAum7o4D8nR4NJacB3NfPnS9tsJ2L3p5iHviuTB4xm03IKmPPq\
             saJy+nXUFC1XS9E/PseVHRuNvKa7WmlwSZngQzKAVSIwqpgCc+oP1pKEe\
             J0M3LHFo8ao5SuzhfXUIGrPnkUKEE3m7B0b8xXZfP1N6ELoonWDK+RMgY\
             IBaZdgBhPfHxF8KfTHvSzcUzWZojuR+ynaFL9AJK+8RiXnB4CJwIDAQAB",
        )
        .expect("invalid base64");

        // RSA algorithm identifier

        let algorithm = AlgorithmIdentifier::new_rsa_encryption();
        check_serde!(algorithm: AlgorithmIdentifier in encoded[4..19]);

        // RSA modulus and public exponent

        let modulus = IntegerAsn1::from_bytes_be_signed(vec![
            0x00, 0xb2, 0x22, 0xe8, 0x23, 0x19, 0x97, 0x69, 0x90, 0x5, 0x44, 0x12, 0xad, 0x1d, 0x86, 0x61, 0x88, 0x5f,
            0x26, 0xfa, 0x96, 0x11, 0xfb, 0x11, 0x88, 0xa6, 0xea, 0x6f, 0xb1, 0xd0, 0xc4, 0xbc, 0xa3, 0xbd, 0xd8, 0x3e,
            0x9f, 0x21, 0xc7, 0x20, 0x54, 0xb2, 0x16, 0xea, 0x9e, 0xac, 0x48, 0xd9, 0x28, 0x5f, 0x1e, 0x49, 0xe8, 0xa0,
            0xd2, 0x31, 0xb0, 0x3f, 0x72, 0x1c, 0xb9, 0x33, 0xaa, 0x51, 0xbc, 0x99, 0x1a, 0x9, 0x70, 0x88, 0xe3, 0x39,
            0x42, 0x3c, 0xc6, 0x2f, 0x3e, 0x12, 0x3c, 0x3b, 0x13, 0x77, 0x45, 0x83, 0xdf, 0x75, 0x4, 0xcb, 0xa6, 0x3,
            0xf2, 0xc2, 0xb9, 0x5f, 0x76, 0x7d, 0x82, 0x2, 0xe9, 0xbb, 0xa3, 0x80, 0xfc, 0x9d, 0x1e, 0xd, 0x25, 0xa7,
            0x1, 0xdc, 0xd7, 0xcf, 0x9d, 0x2f, 0x6d, 0xb0, 0x9d, 0x8b, 0xde, 0x9e, 0x62, 0x1e, 0xf8, 0xae, 0x4c, 0x1e,
            0x31, 0x9b, 0x4d, 0xc8, 0x2a, 0x63, 0xcf, 0xaa, 0xc6, 0x89, 0xcb, 0xe9, 0xd7, 0x50, 0x50, 0xb5, 0x5d, 0x2f,
            0x44, 0xfc, 0xfb, 0x1e, 0x54, 0x74, 0x6e, 0x36, 0xf2, 0x9a, 0xed, 0x69, 0xa5, 0xc1, 0x26, 0x67, 0x81, 0xc,
            0xca, 0x1, 0x54, 0x88, 0xc2, 0xaa, 0x60, 0x9, 0xcf, 0xa8, 0x3f, 0x5a, 0x4a, 0x11, 0xe2, 0x74, 0x33, 0x72,
            0xc7, 0x16, 0x8f, 0x1a, 0xa3, 0x94, 0xae, 0xce, 0x17, 0xd7, 0x50, 0x81, 0xab, 0x3e, 0x79, 0x14, 0x28, 0x41,
            0x37, 0x9b, 0xb0, 0x74, 0x6f, 0xcc, 0x57, 0x65, 0xf3, 0xf5, 0x37, 0xa1, 0xb, 0xa2, 0x89, 0xd6, 0xc, 0xaf,
            0x91, 0x32, 0x6, 0x8, 0x5, 0xa6, 0x5d, 0x80, 0x18, 0x4f, 0x7c, 0x7c, 0x45, 0xf0, 0xa7, 0xd3, 0x1e, 0xf4,
            0xb3, 0x71, 0x4c, 0xd6, 0x66, 0x88, 0xee, 0x47, 0xec, 0xa7, 0x68, 0x52, 0xfd, 0x0, 0x92, 0xbe, 0xf1, 0x18,
            0x97, 0x9c, 0x1e, 0x2, 0x27,
        ]);
        check_serde!(modulus: IntegerAsn1 in encoded[28..289]);

        let public_exponent: IntegerAsn1 = BigInt::from(65537).to_signed_bytes_be().into();
        check_serde!(public_exponent: IntegerAsn1 in encoded[289..294]);

        // RSA public key

        let subject_public_key: EncapsulatedRsaPublicKey = RsaPublicKey {
            modulus,
            public_exponent,
        }
        .into();
        check_serde!(subject_public_key: EncapsulatedRsaPublicKey in encoded[19..294]);

        // full encode / decode

        let info = SubjectPublicKeyInfo {
            algorithm,
            subject_public_key: PublicKey::Rsa(subject_public_key),
        };
        check_serde!(info: SubjectPublicKeyInfo in encoded);
    }
}
