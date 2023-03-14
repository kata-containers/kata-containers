use crate::{
    errors::{Error, Result},
    RSAPrivateKey, RSAPublicKey,
};
use simple_asn1::{ASN1Block, ASN1DecodeErr, BigUint, OID};

use std::convert::TryFrom;

impl From<ASN1DecodeErr> for Error {
    fn from(e: ASN1DecodeErr) -> Error {
        Error::ParseError {
            reason: format!("{}", e),
        }
    }
}

#[cfg(feature = "pem")]
impl TryFrom<pem::Pem> for RSAPrivateKey {
    type Error = Error;

    /// Parses a `PKCS8` or `PKCS1` encoded RSA Private Key.
    ///
    /// Expects one of the following `pem` headers:
    /// - `-----BEGIN PRIVATE KEY-----`
    /// - `-----BEGIN RSA PRIVATE KEY-----`
    ///
    /// # Example
    ///
    /// ```
    /// use std::convert::TryFrom;
    /// use rsa::RSAPrivateKey;
    ///
    /// # // openssl genrsa -out tiny_key.pem 512
    /// let file_content = r#"
    /// -----BEGIN RSA PRIVATE KEY-----
    /// MIIBOwIBAAJBAK5Z7jk1ql5DquRvlPmFgyBDCvdPQ0T2si2oPAUmNw2Z/qb2Sr/B
    /// EBoWpagFf8Gl1K4PRipJSudDl6N/Vdb2CYkCAwEAAQJBAI3vWCfqsE8c9zoQPE8F
    /// icHx0jOSq0ixLExO8M2gVqESq3SJpWbEbvPPbRb1sIqZHe5wV3Xmj09zvUzfdeB7
    /// C6ECIQDjoB/kp7QlRiNhgudhQPct8XUf6Cgp7hBxL2K9Q9UzawIhAMQVvtH1TUOd
    /// aSWiqrFx7w+54o58fIpkecI5Kl0TaWfbAiBrnye1Kn2IKhNMZWIUn2y+8izYeyGS
    /// QZbQjQD4T3wcJQIgKGgWv2teNZ29ai0AIbrJuaLjhdsvStFzqctf6Hg0k1sCIQCj
    /// JdwDGF7Kanex70KAacmOlw3vfx6XWT+2PH6Qh8tLug==
    /// -----END RSA PRIVATE KEY-----
    /// "#;
    ///
    /// let pem = rsa::pem::parse(file_content).expect("failed to parse pem file");
    /// let private_key = RSAPrivateKey::try_from(pem).expect("failed to parse key");
    /// ```
    fn try_from(pem: pem::Pem) -> Result<RSAPrivateKey> {
        match &*pem.tag {
            "RSA PRIVATE KEY" => parse_private_key_pkcs1(&pem.contents),
            "PRIVATE KEY" => parse_private_key_pkcs8(&pem.contents),
            _ => Err(Error::ParseError {
                reason: format!("unexpected tag: {}", pem.tag),
            }),
        }
    }
}

#[cfg(feature = "pem")]
impl TryFrom<pem::Pem> for RSAPublicKey {
    type Error = Error;

    /// Parses a `PKCS8` or `PKCS1` encoded RSA Public Key.
    ///
    /// Expects one of the following `pem` headers:
    /// - `-----BEGIN PUBLIC KEY-----`
    /// - `-----BEGIN RSA PUBLIC KEY-----`
    ///
    /// # Example
    ///
    /// ```
    /// use std::convert::TryFrom;
    /// use rsa::RSAPublicKey;
    ///
    /// # // openssl rsa -in tiny_key.pem -outform PEM -pubout -out tiny_key.pub.pem
    /// let file_content = r#"
    /// -----BEGIN PUBLIC KEY-----
    /// MFwwDQYJKoZIhvcNAQEBBQADSwAwSAJBAK5Z7jk1ql5DquRvlPmFgyBDCvdPQ0T2
    /// si2oPAUmNw2Z/qb2Sr/BEBoWpagFf8Gl1K4PRipJSudDl6N/Vdb2CYkCAwEAAQ==
    /// -----END PUBLIC KEY-----
    /// "#;
    ///
    /// let pem = rsa::pem::parse(file_content).expect("failed to parse pem file");
    /// let public_key = RSAPublicKey::try_from(pem).expect("failed to parse key");
    /// ```
    fn try_from(pem: pem::Pem) -> Result<RSAPublicKey> {
        match &*pem.tag {
            "RSA PUBLIC KEY" => parse_public_key_pkcs1(&pem.contents),
            "PUBLIC KEY" => parse_public_key_pkcs8(&pem.contents),
            _ => Err(Error::ParseError {
                reason: format!("unexpected tag: {}", pem.tag),
            }),
        }
    }
}

fn big_uint(value: &simple_asn1::BigInt) -> Result<crate::BigUint> {
    match value.to_biguint() {
        Some(value) => {
            // TODO: Open simple_asn1 pull request to update the num-bigint crate
            Ok(crate::BigUint::from_bytes_le(&value.to_bytes_le()))
        }
        None => Err(Error::ParseError {
            reason: format!("BigInt::to_biguint failed"),
        }),
    }
}

macro_rules! try_asn1 {
    (Sequence($maybe:expr)) => {
        if let Some(ASN1Block::Sequence(_, value)) = $maybe {
            value
        } else {
            return Err(Error::ParseError {
                reason: format!("expected asn1 sequence"),
            });
        }
    };
    (Integer($maybe:expr), $field:expr) => {
        if let Some(ASN1Block::Integer(_, value)) = $maybe {
            value
        } else {
            return Err(Error::ParseError {
                reason: format!("expected asn1 integer: {}", $field),
            });
        }
    };
    (BitString($maybe:expr), $field:expr) => {
        if let Some(ASN1Block::BitString(_, _, value)) = $maybe {
            value
        } else {
            return Err(Error::ParseError {
                reason: format!("expected asn1 bit string: {}", $field),
            });
        }
    };
    (OctetString($maybe:expr), $field:expr) => {
        if let Some(ASN1Block::OctetString(_, value)) = $maybe {
            value
        } else {
            return Err(Error::ParseError {
                reason: format!("expected asn1 octet string: {}", $field),
            });
        }
    };
    (ObjectIdentifier($maybe:expr), $field:expr) => {
        if let Some(ASN1Block::ObjectIdentifier(_, value)) = $maybe {
            value
        } else {
            return Err(Error::ParseError {
                reason: format!("expected asn1 object identifier: {}", $field),
            });
        }
    };
}

/// Parse a `PKCS1` encoded RSA Private Key.
///
/// The `der` data is expected to be the `base64` decoded content
/// following a `-----BEGIN RSA PRIVATE KEY-----` header.
///
/// <https://tls.mbed.org/kb/cryptography/asn1-key-structures-in-der-and-pem>
pub fn parse_private_key_pkcs1(der: &[u8]) -> Result<RSAPrivateKey> {
    let asn1 = simple_asn1::from_der(der)?;
    let sequence = try_asn1!(Sequence(asn1.iter().next()));
    let mut blocks = sequence.iter();
    let _version = big_uint(try_asn1!(Integer(blocks.next()), "version"))?;
    let n = big_uint(try_asn1!(Integer(blocks.next()), "modulus (n)"))?;
    let e = big_uint(try_asn1!(Integer(blocks.next()), "publicExponent (e)"))?;
    let d = big_uint(try_asn1!(Integer(blocks.next()), "privateExponent (d)"))?;
    let prime1 = big_uint(try_asn1!(Integer(blocks.next()), "prime1"))?;
    let prime2 = big_uint(try_asn1!(Integer(blocks.next()), "prime2"))?;
    let primes = vec![prime1, prime2];
    Ok(RSAPrivateKey::from_components(n, e, d, primes))
}

/// Parse a `PKCS1` encoded RSA Public Key.
///
/// The `der` data is expected to be the `base64` decoded content
/// following a `-----BEGIN RSA PUBLIC KEY-----` header.
///
/// <https://tls.mbed.org/kb/cryptography/asn1-key-structures-in-der-and-pem>
pub fn parse_public_key_pkcs1(der: &[u8]) -> Result<RSAPublicKey> {
    let asn1 = simple_asn1::from_der(der)?;
    let sequence = try_asn1!(Sequence(asn1.iter().next()));
    let mut blocks = sequence.iter();
    let n = big_uint(try_asn1!(Integer(blocks.next()), "modulus (n)"))?;
    let e = big_uint(try_asn1!(Integer(blocks.next()), "exponent (e)"))?;
    Ok(RSAPublicKey::new(n, e)?)
}

/// Parse a `PKCS8` encoded RSA Public Key.
///
/// The `der` data is expected to be the `base64` decoded content
/// following a `-----BEGIN PUBLIC KEY-----` header.
///
/// <https://tls.mbed.org/kb/cryptography/asn1-key-structures-in-der-and-pem>
pub fn parse_public_key_pkcs8(der: &[u8]) -> Result<RSAPublicKey> {
    let asn1 = simple_asn1::from_der(der)?;
    let sequence = try_asn1!(Sequence(asn1.iter().next()));
    let mut blocks = sequence.iter();
    let algorithm = try_asn1!(Sequence(blocks.next()));
    let oid = try_asn1!(ObjectIdentifier(algorithm.iter().next()), "oid");

    if oid != rsa_oid() {
        return Err(Error::ParseError {
            reason: format!("oid mismatch: not an rsa key"),
        });
    }

    let bit_string = try_asn1!(BitString(blocks.next()), "PublicKey");

    parse_public_key_pkcs1(&bit_string)
}

/// Parse a `PKCS8` encoded RSA Private Key.
///
/// The `der` data is expected to be the `base64` decoded content
/// following a `-----BEGIN PRIVATE KEY-----` header.
///
/// <https://tls.mbed.org/kb/cryptography/asn1-key-structures-in-der-and-pem>
pub fn parse_private_key_pkcs8(der: &[u8]) -> Result<RSAPrivateKey> {
    let asn1 = simple_asn1::from_der(der)?;
    let sequence = try_asn1!(Sequence(asn1.iter().next()));
    let mut blocks = sequence.iter();
    let _version = big_uint(try_asn1!(Integer(blocks.next()), "version"))?;
    let algorithm = try_asn1!(Sequence(blocks.next()));
    let oid = try_asn1!(ObjectIdentifier(algorithm.iter().next()), "oid");

    if oid != rsa_oid() {
        return Err(Error::ParseError {
            reason: format!("oid mismatch: not an rsa key"),
        });
    }

    let octet_string = try_asn1!(OctetString(blocks.next()), "PrivateKey");

    parse_private_key_pkcs1(&octet_string)
}

fn rsa_oid() -> OID {
    simple_asn1::oid!(1, 2, 840, 113549, 1, 1, 1)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_private_key_pkcs1, parse_private_key_pkcs8, parse_public_key_pkcs1,
        parse_public_key_pkcs8,
    };
    use crate::{PaddingScheme, PublicKey, RSAPrivateKey, RSAPublicKey};

    #[cfg(feature = "pem")]
    use std::convert::TryFrom;

    /// `openssl genrsa -out test_key.pem`
    const PKCS1_PRIVATE_KEY: &str = r#"
-----BEGIN RSA PRIVATE KEY-----
MIIEogIBAAKCAQEAty3qrJz+cIhdfghtfD+f9CW+H6rn6IYyHQxYuDoie6eeHuRg
m8xAn/l37CcyBh/EkQz5WqD7DD5koK0jsHFWz40WC6USh+W7+1TMFJrp8orF+F8U
PeXlydTo2B7fXvIyi1oOF0HpgcRF/L2Ey/2MLvDmY+uLxil/YJNtF/BI57ycaphQ
LezlRsTpvwAwruLCDeBRefqw39u8nDiOVTr0bLfSzSQtHbX+Fxnko+MzLA3tdWGH
wv3tiPnTyLr+FIzzuJtX7aTuqzMM+u3sH7eKS6M1Lu0qFRw5LFcagLgWsIDUmP+5
hsrzVY8J0wvbOtL9NjCMy6zz1zQ+N7oJ9mdhNwIDAQABAoIBAHtWWF+3KX7d4o18
4TM6p9m9HAG4koO278EtUgNhaVx3JPlJ7l6YrZ7JW1zPm1gSRckgwjiqkb7Rt/GU
AqbH+ZqwNXrLv+lu3x7AHtV05TbhB6FPa5Kt3AYE7G6wgtgsHapEjZ5NTAuK+1DM
zsCHTL9Chu4aaDeaM2D0gw0ORhh5ex4S+NW0utGdkKXP85rxrhqMj58T0HuMPqGK
TyPQzXX58nJ/OniYs4iPJUKV+iiN0FIhQUDn7dcNi3o3nqNdULNMp9t91EePqe4N
XIPVmfYSOK7nsB3n9dK6JRKuXA2PwFfPWaYBYsb2O3MH80u7qTYoxrcoVzkluaEt
IINxF3kCgYEA6f6thnq9+CYvWf8/cYDTs6uTX/zNHravsFUH1SBTUXBMrj26qZ65
tbnWmqZUwbOHb/qvcIBR4gqZW8fMIKMimZoSdqAEMEhOOiak/fqG7asnsFmkryzL
s8Vu7EfZWMTHHpVap5tOBo2l3Z9wFXV+sUY6woMLhcKycctDBjMY9lsCgYEAyGfg
+mqdm91TcOaOJO0WPPNm+S8MKzfRBD6NZcCUn08mg+GLhBysX3hMpdWlnxiU04PR
Z6B2e1EDXrppBVnohXPTSdUpsxU1Bot+GN2sxn3wjvkrLUrbd5K7L463nNvfCbnI
Xnw0YpDBqxVSmllRgadlVE5bNQsY7tyRRhfez1UCgYA08iB3dlx3wsQiHARR/XFp
jSAarwwGsBWO056jFd5kZgGjx2nuKXEh8nvhoFM7RREXQGTkEtT0TaunvcytcdxJ
XzhgSRLJjgLNW6MnqMFRE9I2MAJ4dK1e7wSLSDKgyF25yNerZxO/ndtzCzmEUYKq
QBbZnmdEC+runqx6waMbUwKBgH/yGiemQ8OL9UbCW4Plvenh+B8k018QPqi4Cuwo
nHptPQi7yafp6n71PfIuSZQeTH4RzXKcdqhaW41x87TP5uy0uvOLXRkRdA4epr3X
YQREyX1uRQugnCdmDY3aTw2dLnN2Ih94qrU29/5zGY6jca8WIVJGyGJAGIX/Wdxn
RwAVAoGAbKBEaO52DpjLXc0fdSuYtYCxBdoAhuDFcrHBeStxyndzIbIyVkHLpui9
8Fx/mzEXc+yfT+tqY2q3nFUDKCj9fby62rbnQOs+7tdBx4qGVA1VymAcrfB3vIGt
Kw1Dq0JU0uu70Ivod7tZLZd++lQlBaONOCLXN7csQE7EcviQrxI=
-----END RSA PRIVATE KEY-----
"#;

    /// `openssl pkcs8 -topk8 -inform PEM -outform PEM -in test_key.pem -out test_key.pkcs8.pem -nocrypt`
    const PKCS8_PRIVATE_KEY: &str = r#"
-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQC3LeqsnP5wiF1+
CG18P5/0Jb4fqufohjIdDFi4OiJ7p54e5GCbzECf+XfsJzIGH8SRDPlaoPsMPmSg
rSOwcVbPjRYLpRKH5bv7VMwUmunyisX4XxQ95eXJ1OjYHt9e8jKLWg4XQemBxEX8
vYTL/Ywu8OZj64vGKX9gk20X8EjnvJxqmFAt7OVGxOm/ADCu4sIN4FF5+rDf27yc
OI5VOvRst9LNJC0dtf4XGeSj4zMsDe11YYfC/e2I+dPIuv4UjPO4m1ftpO6rMwz6
7ewft4pLozUu7SoVHDksVxqAuBawgNSY/7mGyvNVjwnTC9s60v02MIzLrPPXND43
ugn2Z2E3AgMBAAECggEAe1ZYX7cpft3ijXzhMzqn2b0cAbiSg7bvwS1SA2FpXHck
+UnuXpitnslbXM+bWBJFySDCOKqRvtG38ZQCpsf5mrA1esu/6W7fHsAe1XTlNuEH
oU9rkq3cBgTsbrCC2CwdqkSNnk1MC4r7UMzOwIdMv0KG7hpoN5ozYPSDDQ5GGHl7
HhL41bS60Z2Qpc/zmvGuGoyPnxPQe4w+oYpPI9DNdfnycn86eJiziI8lQpX6KI3Q
UiFBQOft1w2Lejeeo11Qs0yn233UR4+p7g1cg9WZ9hI4ruewHef10rolEq5cDY/A
V89ZpgFixvY7cwfzS7upNijGtyhXOSW5oS0gg3EXeQKBgQDp/q2Ger34Ji9Z/z9x
gNOzq5Nf/M0etq+wVQfVIFNRcEyuPbqpnrm1udaaplTBs4dv+q9wgFHiCplbx8wg
oyKZmhJ2oAQwSE46JqT9+obtqyewWaSvLMuzxW7sR9lYxMcelVqnm04GjaXdn3AV
dX6xRjrCgwuFwrJxy0MGMxj2WwKBgQDIZ+D6ap2b3VNw5o4k7RY882b5LwwrN9EE
Po1lwJSfTyaD4YuEHKxfeEyl1aWfGJTTg9FnoHZ7UQNeumkFWeiFc9NJ1SmzFTUG
i34Y3azGffCO+SstStt3krsvjrec298JuchefDRikMGrFVKaWVGBp2VUTls1Cxju
3JFGF97PVQKBgDTyIHd2XHfCxCIcBFH9cWmNIBqvDAawFY7TnqMV3mRmAaPHae4p
cSHye+GgUztFERdAZOQS1PRNq6e9zK1x3ElfOGBJEsmOAs1boyeowVET0jYwAnh0
rV7vBItIMqDIXbnI16tnE7+d23MLOYRRgqpAFtmeZ0QL6u6erHrBoxtTAoGAf/Ia
J6ZDw4v1RsJbg+W96eH4HyTTXxA+qLgK7Cicem09CLvJp+nqfvU98i5JlB5MfhHN
cpx2qFpbjXHztM/m7LS684tdGRF0Dh6mvddhBETJfW5FC6CcJ2YNjdpPDZ0uc3Yi
H3iqtTb3/nMZjqNxrxYhUkbIYkAYhf9Z3GdHABUCgYBsoERo7nYOmMtdzR91K5i1
gLEF2gCG4MVyscF5K3HKd3MhsjJWQcum6L3wXH+bMRdz7J9P62pjarecVQMoKP19
vLratudA6z7u10HHioZUDVXKYByt8He8ga0rDUOrQlTS67vQi+h3u1ktl376VCUF
o404Itc3tyxATsRy+JCvEg==
-----END PRIVATE KEY-----
"#;

    /// `openssl rsa -in test_key.pem -outform PEM -pubout -out test_key.pub.pem`
    const PKCS8_PUBLIC_KEY: &str = r#"
-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAty3qrJz+cIhdfghtfD+f
9CW+H6rn6IYyHQxYuDoie6eeHuRgm8xAn/l37CcyBh/EkQz5WqD7DD5koK0jsHFW
z40WC6USh+W7+1TMFJrp8orF+F8UPeXlydTo2B7fXvIyi1oOF0HpgcRF/L2Ey/2M
LvDmY+uLxil/YJNtF/BI57ycaphQLezlRsTpvwAwruLCDeBRefqw39u8nDiOVTr0
bLfSzSQtHbX+Fxnko+MzLA3tdWGHwv3tiPnTyLr+FIzzuJtX7aTuqzMM+u3sH7eK
S6M1Lu0qFRw5LFcagLgWsIDUmP+5hsrzVY8J0wvbOtL9NjCMy6zz1zQ+N7oJ9mdh
NwIDAQAB
-----END PUBLIC KEY-----
"#;

    /// `ssh-keygen -t rsa -e -m pem -f test_key.pem`
    const PKCS1_PUBLIC_KEY: &str = r#"
-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAty3qrJz+cIhdfghtfD+f9CW+H6rn6IYyHQxYuDoie6eeHuRgm8xA
n/l37CcyBh/EkQz5WqD7DD5koK0jsHFWz40WC6USh+W7+1TMFJrp8orF+F8UPeXl
ydTo2B7fXvIyi1oOF0HpgcRF/L2Ey/2MLvDmY+uLxil/YJNtF/BI57ycaphQLezl
RsTpvwAwruLCDeBRefqw39u8nDiOVTr0bLfSzSQtHbX+Fxnko+MzLA3tdWGHwv3t
iPnTyLr+FIzzuJtX7aTuqzMM+u3sH7eKS6M1Lu0qFRw5LFcagLgWsIDUmP+5hsrz
VY8J0wvbOtL9NjCMy6zz1zQ+N7oJ9mdhNwIDAQAB
-----END RSA PUBLIC KEY-----
"#;

    #[test]
    fn parse_pkcs1_private_key() {
        let pem = pem::parse(PKCS1_PRIVATE_KEY).expect("pem::parse failed");
        parse_private_key_pkcs1(&pem.contents).expect("parse_private_key_pkcs1 failed");
    }

    #[test]
    fn parse_pkcs8_private_key() {
        let pem = pem::parse(PKCS8_PRIVATE_KEY).expect("pem::parse failed");
        parse_private_key_pkcs8(&pem.contents).expect("parse_private_key_pkcs8 failed");
    }
    #[test]
    fn parse_pkcs1_public_key() {
        let pem = pem::parse(PKCS1_PUBLIC_KEY).expect("pem::parse failed");
        parse_public_key_pkcs1(&pem.contents).expect("parse_public_key_pkcs1 failed");
    }

    #[test]
    fn parse_pkcs8_public_key() {
        let pem = pem::parse(PKCS8_PUBLIC_KEY).expect("pem::parse failed");
        parse_public_key_pkcs8(&pem.contents).expect("parse_public_key_pkcs8 failed");
    }

    #[test]
    fn verify_public_keys_are_equal() {
        let pem = pem::parse(PKCS1_PUBLIC_KEY).expect("pem::parse failed");
        let public_key_pkcs1 =
            parse_public_key_pkcs1(&pem.contents).expect("parse_public_key_pkcs1 failed");

        let pem = pem::parse(PKCS8_PUBLIC_KEY).expect("pem::parse failed");
        let public_key_pkcs8 =
            parse_public_key_pkcs8(&pem.contents).expect("parse_public_key_pkcs8 failed");

        assert_eq!(public_key_pkcs1, public_key_pkcs8);
    }

    #[test]
    fn verify_private_keys_are_equal() {
        let pem = pem::parse(PKCS1_PRIVATE_KEY).expect("pem::parse failed");
        let private_key_pkcs1 =
            parse_private_key_pkcs1(&pem.contents).expect("parse_private_key_pkcs1 failed");

        let pem = pem::parse(PKCS8_PRIVATE_KEY).expect("pem::parse failed");
        let private_key_pkcs8 =
            parse_private_key_pkcs8(&pem.contents).expect("parse_private_key_pkcs8 failed");

        assert_eq!(private_key_pkcs1, private_key_pkcs8);
    }
    #[test]
    fn verify_encrypt_decrypt() {
        let pem = pem::parse(PKCS1_PUBLIC_KEY).expect("pem::parse failed");
        let public_key =
            parse_public_key_pkcs1(&pem.contents).expect("parse_public_key_pkcs1 failed");

        let pem = pem::parse(PKCS1_PRIVATE_KEY).expect("pem::parse failed");
        let private_key =
            parse_private_key_pkcs1(&pem.contents).expect("parse_private_key_pkcs1 failed");

        let rng = &mut rand::thread_rng();
        let clear_text = "Hello, World!";

        let encrypted = public_key
            .encrypt(
                rng,
                PaddingScheme::new_pkcs1v15_encrypt(),
                clear_text.as_bytes(),
            )
            .expect("encrypt failed");

        let decrypted = private_key
            .decrypt(PaddingScheme::new_pkcs1v15_encrypt(), &encrypted)
            .expect("decrypt failed");

        assert_eq!(
            clear_text.as_bytes(),
            decrypted.as_slice(),
            "clear text did not match decrypted data"
        );
    }

    #[test]
    #[cfg(feature = "pem")]
    fn rsa_private_key_try_from_pkcs1() {
        let pem = pem::parse(PKCS1_PRIVATE_KEY).expect("pem::parse failed");
        RSAPrivateKey::try_from(pem).expect("RSAPrivateKey::try_from failed");
    }

    #[test]
    #[cfg(feature = "pem")]
    fn rsa_private_key_try_from_pkcs8() {
        let pem = pem::parse(PKCS8_PRIVATE_KEY).expect("pem::parse failed");
        RSAPrivateKey::try_from(pem).expect("RSAPrivateKey::try_from failed");
    }

    #[test]
    #[cfg(feature = "pem")]
    fn rsa_public_key_try_from_pkcs1() {
        let pem = pem::parse(PKCS1_PUBLIC_KEY).expect("pem::parse failed");
        RSAPublicKey::try_from(pem).expect("RSAPublicKey::try_from failed");
    }

    #[test]
    #[cfg(feature = "pem")]
    fn rsa_public_key_try_from_pkcs8() {
        let pem = pem::parse(PKCS8_PUBLIC_KEY).expect("pem::parse failed");
        RSAPublicKey::try_from(pem).expect("RSAPublicKey::try_from failed");
    }
}
