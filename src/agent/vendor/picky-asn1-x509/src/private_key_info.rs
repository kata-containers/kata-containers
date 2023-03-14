use crate::{oids, AlgorithmIdentifier, AlgorithmIdentifierParameters, EcParameters};
use picky_asn1::wrapper::{
    BitStringAsn1, ExplicitContextTag0, ExplicitContextTag1, IntegerAsn1, OctetStringAsn1, OctetStringAsn1Container,
    Optional,
};
#[cfg(not(feature = "legacy"))]
use serde::Deserialize;
use serde::{de, ser, Serialize};

use std::fmt;
#[cfg(feature = "zeroize")]
use zeroize::Zeroize;

/// [Public-Key Cryptography Standards (PKCS) #8](https://tools.ietf.org/html/rfc5208#section-5)
///
/// # Section 5
///
/// Private-key information shall have ASN.1 type PrivateKeyInfo:
///
/// ```not_rust
/// PrivateKeyInfo ::= SEQUENCE {
///      version                   Version,
///      privateKeyAlgorithm       PrivateKeyAlgorithmIdentifier,
///      privateKey                PrivateKey,
///      attributes           [0]  IMPLICIT Attributes OPTIONAL }
///
///   Version ::= INTEGER
///
///   PrivateKeyAlgorithmIdentifier ::= AlgorithmIdentifier
///
///   PrivateKey ::= OCTET STRING
///
///   Attributes ::= SET OF Attribute
/// ```
///
/// The fields of type PrivateKeyInfo have the following meanings:
///
/// `version` is the syntax version number, for compatibility with
/// future revisions of this document.  It shall be 0 for this version
/// of the document.
///
/// `privateKeyAlgorithm` identifies the private-key algorithm.  One
/// example of a private-key algorithm is PKCS #1's rsaEncryption.
///
/// `privateKey` is an octet string whose contents are the value of the
/// private key.  The interpretation of the contents is defined in the
/// registration of the private-key algorithm.  For an RSA private
/// key, for example, the contents are a BER encoding of a value of
/// type RSAPrivateKey.
///
/// `attributes` is a set of attributes.  These are the extended
/// information that is encrypted along with the private-key
/// information.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct PrivateKeyInfo {
    pub version: u8,
    pub private_key_algorithm: AlgorithmIdentifier,
    pub private_key: PrivateKeyValue,
    //pub attributes
}

impl PrivateKeyInfo {
    pub fn new_rsa_encryption(
        modulus: IntegerAsn1,
        public_exponent: IntegerAsn1,
        private_exponent: IntegerAsn1,
        primes: (IntegerAsn1, IntegerAsn1),
        exponents: (IntegerAsn1, IntegerAsn1),
        coefficient: IntegerAsn1,
    ) -> Self {
        let private_key = PrivateKeyValue::RSA(
            RsaPrivateKey {
                version: vec![0].into(),
                modulus,
                public_exponent,
                private_exponent,
                prime_1: primes.0,
                prime_2: primes.1,
                exponent_1: exponents.0,
                exponent_2: exponents.1,
                coefficient,
            }
            .into(),
        );

        Self {
            version: 0,
            private_key_algorithm: AlgorithmIdentifier::new_rsa_encryption(),
            private_key,
        }
    }
}

impl<'de> de::Deserialize<'de> for PrivateKeyInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = PrivateKeyInfo;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded PrivateKeyInfo (pkcs8)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let version = seq_next_element!(seq, PrivateKeyInfo, "version");
                if version != 0 {
                    return Err(serde_invalid_value!(
                        PrivateKeyInfo,
                        "unsupported version (valid version number: 0)",
                        "a supported PrivateKeyInfo"
                    ));
                }

                let private_key_algorithm: AlgorithmIdentifier =
                    seq_next_element!(seq, PrivateKeyInfo, "private key algorithm");
                let private_key = if private_key_algorithm.is_a(oids::rsa_encryption()) {
                    PrivateKeyValue::RSA(seq_next_element!(seq, PrivateKeyInfo, "rsa oid"))
                } else if matches!(private_key_algorithm.parameters(), AlgorithmIdentifierParameters::Ec(_)) {
                    PrivateKeyValue::EC(seq_next_element!(seq, PrivateKeyInfo, "ec oid"))
                } else {
                    return Err(serde_invalid_value!(
                        PrivateKeyInfo,
                        "unsupported algorithm",
                        "a supported algorithm"
                    ));
                };

                Ok(PrivateKeyInfo {
                    version,
                    private_key_algorithm,
                    private_key,
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum PrivateKeyValue {
    RSA(OctetStringAsn1Container<RsaPrivateKey>),
    EC(OctetStringAsn1Container<ECPrivateKey>),
}

impl ser::Serialize for PrivateKeyValue {
    fn serialize<S>(&self, serializer: S) -> Result<<S as ser::Serializer>::Ok, <S as ser::Serializer>::Error>
    where
        S: ser::Serializer,
    {
        match self {
            PrivateKeyValue::RSA(rsa) => rsa.serialize(serializer),
            PrivateKeyValue::EC(ec) => ec.serialize(serializer),
        }
    }
}

/// [PKCS #1: RSA Cryptography Specifications Version 2.2](https://tools.ietf.org/html/rfc8017.html#appendix-A.1.2)
///
/// # Section A.1.2
///
/// An RSA private key should be represented with the ASN.1 type RSAPrivateKey:
///
/// ```not_rust
///      RSAPrivateKey ::= SEQUENCE {
///          version           Version,
///          modulus           INTEGER,  -- n
///          publicExponent    INTEGER,  -- e
///          privateExponent   INTEGER,  -- d
///          prime1            INTEGER,  -- p
///          prime2            INTEGER,  -- q
///          exponent1         INTEGER,  -- d mod (p-1)
///          exponent2         INTEGER,  -- d mod (q-1)
///          coefficient       INTEGER,  -- (inverse of q) mod p
///          otherPrimeInfos   OtherPrimeInfos OPTIONAL
///      }
/// ```
#[derive(Serialize, Debug, Clone, PartialEq)]
#[cfg_attr(not(feature = "legacy"), derive(Deserialize))]
pub struct RsaPrivateKey {
    pub version: IntegerAsn1,
    pub modulus: IntegerAsn1,
    pub public_exponent: IntegerAsn1,
    pub private_exponent: IntegerAsn1,
    pub prime_1: IntegerAsn1,
    pub prime_2: IntegerAsn1,
    pub exponent_1: IntegerAsn1,
    pub exponent_2: IntegerAsn1,
    pub coefficient: IntegerAsn1,
}

#[cfg(feature = "zeroize")]
impl Drop for RsaPrivateKey {
    fn drop(&mut self) {
        self.private_exponent.zeroize();
    }
}

#[cfg(feature = "legacy")]
impl<'de> de::Deserialize<'de> for RsaPrivateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = RsaPrivateKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct RSAPrivateKey with 6 or 9 elements")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let version: IntegerAsn1 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let modulus: IntegerAsn1 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let public_exponent: IntegerAsn1 =
                    seq.next_element()?.ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let private_exponent: IntegerAsn1 =
                    seq.next_element()?.ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let prime_1: IntegerAsn1 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let prime_2: IntegerAsn1 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(5, &self))?;

                let (exponent_1, exponent_2, coefficient) = if let Some(exponent_1) = seq.next_element()? {
                    let exponent_2 = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(7, &self))?;
                    let coefficient = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(8, &self))?;
                    (exponent_1, exponent_2, coefficient)
                } else {
                    use num_bigint_dig::{BigUint, ModInverse};

                    // conversion to num_bigint_dig format BigUint
                    let private_exponent = BigUint::from_bytes_be(private_exponent.as_unsigned_bytes_be());
                    let prime_1 = BigUint::from_bytes_be(prime_1.as_unsigned_bytes_be());
                    let prime_2 = BigUint::from_bytes_be(prime_2.as_unsigned_bytes_be());

                    let exponent_1 = &private_exponent % (&prime_1 - 1u8);
                    let exponent_2 = &private_exponent % (&prime_2 - 1u8);

                    let coefficient = prime_2
                        .mod_inverse(prime_1)
                        .ok_or_else(|| {
                            de::Error::invalid_value(
                                de::Unexpected::Other("[RSAPrivateKey] no modular inverse for prime 1"),
                                &"an invertible prime 1 value",
                            )
                        })?
                        .to_biguint()
                        .ok_or_else(|| {
                            de::Error::invalid_value(
                                de::Unexpected::Other("[RSAPrivateKey] BigUint conversion failed"),
                                &"a valid prime 1 value",
                            )
                        })?;

                    // conversion to IntegerAsn1
                    let exponent_1 = IntegerAsn1::from_bytes_be_unsigned(exponent_1.to_bytes_be());
                    let exponent_2 = IntegerAsn1::from_bytes_be_unsigned(exponent_2.to_bytes_be());
                    let coefficient = IntegerAsn1::from_bytes_be_unsigned(coefficient.to_bytes_be());

                    (exponent_1, exponent_2, coefficient)
                };

                Ok(RsaPrivateKey {
                    version,
                    modulus,
                    public_exponent,
                    private_exponent,
                    prime_1,
                    prime_2,
                    exponent_1,
                    exponent_2,
                    coefficient,
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

impl RsaPrivateKey {
    #[deprecated(note = "field is now public")]
    pub fn modulus(&self) -> &IntegerAsn1 {
        &self.modulus
    }

    #[deprecated(note = "field is now public")]
    pub fn public_exponent(&self) -> &IntegerAsn1 {
        &self.public_exponent
    }

    #[deprecated(note = "field is now public")]
    pub fn private_exponent(&self) -> &IntegerAsn1 {
        &self.private_exponent
    }

    #[deprecated(note = "field is now public")]
    pub fn prime_1(&self) -> &IntegerAsn1 {
        &self.prime_1
    }

    #[deprecated(note = "field is now public")]
    pub fn prime_2(&self) -> &IntegerAsn1 {
        &self.prime_2
    }

    #[deprecated(note = "field is now public")]
    pub fn primes(&self) -> (&IntegerAsn1, &IntegerAsn1) {
        (&self.prime_1, &self.prime_2)
    }

    #[deprecated(note = "field is now public")]
    pub fn exponent_1(&self) -> &IntegerAsn1 {
        &self.exponent_1
    }

    #[deprecated(note = "field is now public")]
    pub fn exponent_2(&self) -> &IntegerAsn1 {
        &self.exponent_2
    }

    #[deprecated(note = "field is now public")]
    pub fn exponents(&self) -> (&IntegerAsn1, &IntegerAsn1) {
        (&self.exponent_1, &self.exponent_2)
    }

    #[deprecated(note = "field is now public")]
    pub fn coefficient(&self) -> &IntegerAsn1 {
        &self.coefficient
    }

    #[deprecated(note = "field is now public")]
    pub fn into_public_components(mut self) -> (IntegerAsn1, IntegerAsn1) {
        (
            std::mem::take(&mut self.modulus),
            std::mem::take(&mut self.public_exponent),
        )
    }
}

/// [Elliptic Curve Private Key Structure](https://datatracker.ietf.org/doc/html/rfc5915#section-3)
///
/// EC private key information SHALL have ASN.1 type ECPrivateKey:
///
/// ```not_rust
/// ECPrivateKey ::= SEQUENCE {
///      version        INTEGER { ecPrivkeyVer1(1) } (ecPrivkeyVer1),
///      privateKey     OCTET STRING,
///      parameters [0] ECParameters {{ NamedCurve }} OPTIONAL,
///      publicKey  [1] BIT STRING OPTIONAL
///    }
/// ```

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ECPrivateKey {
    pub version: IntegerAsn1,
    pub private_key: OctetStringAsn1,
    #[serde(skip_serializing_if = "Optional::is_default")]
    pub parameters: Optional<ExplicitContextTag0<Option<EcParameters>>>,
    #[serde(skip_serializing_if = "Optional::is_default")]
    pub public_key: Optional<ExplicitContextTag1<BitStringAsn1>>,
}

impl<'de> serde::Deserialize<'de> for ECPrivateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = ECPrivateKey;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded PrivateKeyInfo (pkcs8)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let version: IntegerAsn1 = seq_next_element!(seq, IntegerAsn1, "IntegerAsn1");
                if version.0 != [1] {
                    return Err(serde_invalid_value!(
                        ECPrivateKey,
                        "ECPrivateKey's version is not 1",
                        "ECPrivateKey's version equals to 1"
                    ));
                }

                Ok(ECPrivateKey {
                    version,
                    private_key: seq_next_element!(seq, OctetStringAsn1, "OctetStringAsn1"),
                    parameters: seq_next_element!(
                        seq,
                        Optional<ExplicitContextTag0<Option<EcParameters>>>,
                        ECPrivateKey,
                        "EcParameters"
                    ),
                    public_key: seq_next_element!(
                        seq,
                        Optional<ExplicitContextTag1<BitStringAsn1>>,
                        ECPrivateKey,
                        "BitStringAsn1"
                    ),
                })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

#[cfg(feature = "zeroize")]
impl Drop for ECPrivateKey {
    fn drop(&mut self) {
        self.private_key.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use picky_asn1::bit_string::BitString;

    #[test]
    fn pkcs_8_private_key() {
        let encoded = base64::decode(
            "MIIBVgIBADANBgkqhkiG9w0BAQEFAASCAUAwggE8AgEAAkEAq7BFUpkGp3+LQmlQ\
             Yx2eqzDV+xeG8kx/sQFV18S5JhzGeIJNA72wSeukEPojtqUyX2J0CciPBh7eqclQ\
             2zpAswIDAQABAkAgisq4+zRdrzkwH1ITV1vpytnkO/NiHcnePQiOW0VUybPyHoGM\
             /jf75C5xET7ZQpBe5kx5VHsPZj0CBb3b+wSRAiEA2mPWCBytosIU/ODRfq6EiV04\
             lt6waE7I2uSPqIC20LcCIQDJQYIHQII+3YaPqyhGgqMexuuuGx+lDKD6/Fu/JwPb\
             5QIhAKthiYcYKlL9h8bjDsQhZDUACPasjzdsDEdq8inDyLOFAiEAmCr/tZwA3qeA\
             ZoBzI10DGPIuoKXBd3nk/eBxPkaxlEECIQCNymjsoI7GldtujVnr1qT+3yedLfHK\
             srDVjIT3LsvTqw==",
        )
        .expect("invalid base64");

        let modulus = IntegerAsn1::from(encoded[35..100].to_vec());
        let public_exponent = IntegerAsn1::from(encoded[102..105].to_vec());
        let private_exponent = IntegerAsn1::from(encoded[107..171].to_vec());
        let prime_1 = IntegerAsn1::from(encoded[173..206].to_vec());
        let prime_2 = IntegerAsn1::from(encoded[208..241].to_vec());
        let exponent_1 = IntegerAsn1::from(encoded[243..276].to_vec());
        let exponent_2 = IntegerAsn1::from(encoded[278..311].to_vec());
        let coefficient = IntegerAsn1::from(encoded[313..346].to_vec());

        let private_key = PrivateKeyInfo::new_rsa_encryption(
            modulus,
            public_exponent,
            private_exponent,
            (prime_1, prime_2),
            (exponent_1, exponent_2),
            coefficient,
        );
        check_serde!(private_key: PrivateKeyInfo in encoded);
    }

    #[test]
    #[cfg(feature = "legacy")]
    fn old_broken_key_legacy_support() {
        // Version previous to picky-asn1-x509 6.0.0 could generate weird keys with negative values
        // https://github.com/Devolutions/picky-rs/issues/53
        // We want to support these for now.

        let encoded = base64::decode(
            "MIIDMAIBADANBgkqhkiG9w0BAQEFAASCAxowggMWAgEAAoIBAOB9jOJvCkMHOc98Q\
             GPFikxAvBKANkme5f/nNuNnEnbefoKDFkS6ElfqASAAkIHxUREnRvBTTa6b+qba/0\
             DhBuXsYGCl8VF0pUE4JGujv1HIi5aRCar0WmY66s7DJ4uR3Nk9Jy0WeRiH4yyzEIG\
             8+6QDu4d/U6slWTmE8eZtQEE7rz4FGpQU9OhrGM3xJOIIbLX/xU2SFt83Xs3JREEt\
             bfrXQpSxAHmtwvlBKpeZacrcobm6eQKsoI2MIg3LFvoHs0+40dadm14ngpgwx4qqk\
             bG34jvWH13OhHRweFGNkQpcg99rlzZYkCM13e9EcmirQ9XYHuB5pHS31eznolZKbx\
             cCAwEAAQKCAQCrPFlopxaGxk48jCR5dkbln0NWQWInigMazf06PHcDIPgTCXbE+cH\
             gOWieRo/z7mTN1s3vpztMA0KQX9/wVzVx0Ho7fpiyb21WcEKnsIHRGk4PjZZ4Rmdm\
             L27IRGg3uA1jz5fAdrHsGksY34Wp0MOJ+ibjViY2GAkVLOlvwMoQds6eNIGO88T5O\
             fcmvutjK43ObU1vgx2ptTaLNAVczEE5VHqcLx4GZPv6k71afOQfIDQerIpsGb4gvr\
             1JdwYKb4z02z2SaNIA3Vly0q5s4r8uU36eg9z65utu93M7zI7f8/MX2byZ2Jz4b3T\
             nH10FURmbPoNQH/O2T0TbtT4M1y0xAoGA72JW0IcFxze7j7PPaP6cQN1IXvFDZUFF\
             dZHqFI8+4VPcv3EKTs+iQflM7pqtRuEWtwonIn3f7CGOx317uKwpVsZvfnDhXCUPJ\
             Q3pns7KgaROGXyruFFQ9gl6XsXGK02Wop9nX0/iRK3ruwZ4uJwDioEYcvGw+ocqAc\
             yOdodNnpUCgYDwEo/sPJNaPOzc7fpaQr2PUUJ3ksL0ncGRO2h1JGYgDtWe5u1srSI\
             DlpM2NdYSZyT04ebOF2SqNUBwY3LB1tPOFnjYwCutp4c75OYhOor7TodZHlzt3GeQ\
             ntUw6XbHX0ohTgs4u2NXwOTq5yKeW4VYzuevN5ksF8GoW2noalpn+w==",
        )
        .unwrap();

        picky_asn1_der::from_bytes::<PrivateKeyInfo>(&encoded).unwrap();
    }

    #[test]
    fn decode_ec_key() {
        let decoded = base64::decode(
            "\
        MIHcAgEBBEIBhqphIGu2PmlcEb6xADhhSCpgPUulB0s4L2qOgolRgaBx4fNgINFE\
        mBsSyHJncsWG8WFEuUzAYy/YKz2lP0Qx6Z2gBwYFK4EEACOhgYkDgYYABABwBevJ\
        w/+Xh6I98ruzoTX3MNTsbgnc+glenJRCbEJkjbJrObFhbfgqP52r1lAy2RxuShGi\
        NYJJzNPT6vR1abS32QFtvTH7YbYa6OWk9dtGNY/cYxgx1nQyhUuofdW7qbbfu/Ww\
        TP2oFsPXRAavZCh4AbWUn8bAHmzNRyuJonQBKlQlVQ==",
        )
        .unwrap();

        let ec_key = ECPrivateKey {
            version: IntegerAsn1([1].into()),
            private_key: OctetStringAsn1::from(decoded[8..74].to_vec()),
            parameters: ExplicitContextTag0(Some(EcParameters::NamedCurve(oids::secp521r1().into()))).into(),
            public_key: Optional(ExplicitContextTag1(
                BitString::with_bytes(decoded[90..].to_vec()).into(),
            )),
        };

        check_serde!(ec_key: ECPrivateKey in decoded);
    }

    #[test]
    fn decode_pkcs8_ec_key() {
        let decoded = base64::decode("MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgKZqrmOg/cDZ4tPCn\
                                                            4LROs145nxx+ssufvflL8cROxFmhRANCAARmU90fCSTsncefY7hVeKw1WIg/YQmT\
                                                            4DGJ7nJPZ+WXAd/xxp4c0bHGlIOju/U95ITPN9dAmro7OUTDJpz+rzGW").unwrap();
        let expected_pkcs8_ec_key = PrivateKeyInfo {
            version: 0,
            private_key_algorithm: AlgorithmIdentifier::new_elliptic_curve(EcParameters::NamedCurve(
                oids::secp256r1().into(),
            )),
            private_key: PrivateKeyValue::EC(OctetStringAsn1Container(ECPrivateKey {
                version: IntegerAsn1([1].into()),
                private_key: OctetStringAsn1(decoded[36..68].to_vec()),
                parameters: Optional(Default::default()),
                public_key: Optional(ExplicitContextTag1(
                    BitString::with_bytes(decoded[73..].to_vec()).into(),
                )),
            })),
        };

        check_serde!(expected_pkcs8_ec_key: PrivateKeyInfo in decoded);
    }
}
