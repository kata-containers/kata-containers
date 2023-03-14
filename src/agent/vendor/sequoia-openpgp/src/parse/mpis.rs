//! Functions for parsing MPIs.

use std::io::Read;
use buffered_reader::BufferedReader;
use crate::{
    Result,
    Error,
    PublicKeyAlgorithm,
    SymmetricAlgorithm,
    HashAlgorithm,
};
use crate::types::Curve;
use crate::crypto::mpi::{self, MPI};
use crate::parse::{
    PacketHeaderParser,
    Cookie,
};

impl mpi::PublicKey {
    /// Parses a set of OpenPGP MPIs representing a public key.
    ///
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    pub fn parse<R: Read + Send + Sync>(algo: PublicKeyAlgorithm, reader: R) -> Result<Self>
    {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        let mut php = PacketHeaderParser::new_naked(bio);
        Self::_parse(algo, &mut php)
    }

    /// Parses a set of OpenPGP MPIs representing a public key.
    ///
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    pub(crate) fn _parse<'a, T: 'a + BufferedReader<Cookie>>(
        algo: PublicKeyAlgorithm,
        php: &mut PacketHeaderParser<T>)
        -> Result<Self>
    {
        use crate::PublicKeyAlgorithm::*;

        #[allow(deprecated)]
        match algo {
            RSAEncryptSign | RSAEncrypt | RSASign => {
                let n = MPI::parse("rsa_public_n_len", "rsa_public_n", php)?;
                let e = MPI::parse("rsa_public_e_len", "rsa_public_e", php)?;

                Ok(mpi::PublicKey::RSA { e, n })
            }

            DSA => {
                let p = MPI::parse("dsa_public_p_len", "dsa_public_p", php)?;
                let q = MPI::parse("dsa_public_q_len", "dsa_public_q", php)?;
                let g = MPI::parse("dsa_public_g_len", "dsa_public_g", php)?;
                let y = MPI::parse("dsa_public_y_len", "dsa_public_y", php)?;

                Ok(mpi::PublicKey::DSA {
                    p,
                    q,
                    g,
                    y,
                })
            }

            ElGamalEncrypt | ElGamalEncryptSign => {
                let p = MPI::parse("elgamal_public_p_len", "elgamal_public_p",
                                   php)?;
                let g = MPI::parse("elgamal_public_g_len", "elgamal_public_g",
                                   php)?;
                let y = MPI::parse("elgamal_public_y_len", "elgamal_public_y",
                                   php)?;

                Ok(mpi::PublicKey::ElGamal {
                    p,
                    g,
                    y,
                })
            }

            EdDSA => {
                let curve_len = php.parse_u8("curve_len")? as usize;
                let curve = php.parse_bytes("curve", curve_len)?;
                let q = MPI::parse("eddsa_public_len", "eddsa_public", php)?;

                Ok(mpi::PublicKey::EdDSA {
                    curve: Curve::from_oid(&curve),
                    q
                })
            }

            ECDSA => {
                let curve_len = php.parse_u8("curve_len")? as usize;
                let curve = php.parse_bytes("curve", curve_len)?;
                let q = MPI::parse("ecdsa_public_len", "ecdsa_public", php)?;

                Ok(mpi::PublicKey::ECDSA {
                    curve: Curve::from_oid(&curve),
                    q
                })
            }

            ECDH => {
                let curve_len = php.parse_u8("curve_len")? as usize;
                let curve = php.parse_bytes("curve", curve_len)?;
                let q = MPI::parse("ecdh_public_len", "ecdh_public", php)?;
                let kdf_len = php.parse_u8("kdf_len")?;

                if kdf_len != 3 {
                    return Err(Error::MalformedPacket(
                            "wrong kdf length".into()).into());
                }

                let reserved = php.parse_u8("kdf_reserved")?;
                if reserved != 1 {
                    return Err(Error::MalformedPacket(
                            format!("Reserved kdf field must be 0x01, \
                                     got 0x{:x}", reserved)).into());
                }
                let hash: HashAlgorithm = php.parse_u8("kdf_hash")?.into();
                let sym: SymmetricAlgorithm = php.parse_u8("kek_symm")?.into();

                Ok(mpi::PublicKey::ECDH {
                    curve: Curve::from_oid(&curve),
                    q,
                    hash,
                    sym
                })
            }

            Unknown(_) | Private(_) => {
                let mut mpis = Vec::new();
                while let Ok(mpi) = MPI::parse("unknown_len",
                                               "unknown", php) {
                    mpis.push(mpi);
                }
                let rest = php.parse_bytes_eof("rest")?;

                Ok(mpi::PublicKey::Unknown {
                    mpis: mpis.into_boxed_slice(),
                    rest: rest.into_boxed_slice(),
                })
            }
        }
    }
}

impl mpi::SecretKeyMaterial {
    /// Parses secret key MPIs for `algo` plus their SHA1 checksum.
    ///
    /// Fails if the checksum is wrong.
    pub fn parse_with_checksum<R: Read + Send + Sync>(algo: PublicKeyAlgorithm,
                                        reader: R,
                                        checksum: mpi::SecretKeyChecksum)
                                        -> Result<Self> {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        let mut php = PacketHeaderParser::new_naked(bio);
        Self::_parse(algo, &mut php, Some(checksum))
    }

    /// Parses a set of OpenPGP MPIs representing a secret key.
    ///
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    pub fn parse<R: Read + Send + Sync>(algo: PublicKeyAlgorithm, reader: R) -> Result<Self>
    {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        let mut php = PacketHeaderParser::new_naked(bio);
        Self::_parse(algo, &mut php, None)
    }

    /// Parses a set of OpenPGP MPIs representing a secret key.
    ///
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    pub(crate) fn _parse<'a, T: 'a + BufferedReader<Cookie>>(
        algo: PublicKeyAlgorithm,
        php: &mut PacketHeaderParser<T>,
        checksum: Option<mpi::SecretKeyChecksum>,
    )
        -> Result<Self>
    {
        use crate::PublicKeyAlgorithm::*;

        #[allow(deprecated)]
        let mpis: Result<Self> = match algo {
            RSAEncryptSign | RSAEncrypt | RSASign => {
                let d = MPI::parse("rsa_secret_d_len", "rsa_secret_d", php)?;
                let p = MPI::parse("rsa_secret_p_len", "rsa_secret_p", php)?;
                let q = MPI::parse("rsa_secret_q_len", "rsa_secret_q", php)?;
                let u = MPI::parse("rsa_secret_u_len", "rsa_secret_u", php)?;

                Ok(mpi::SecretKeyMaterial::RSA {
                    d: d.into(),
                    p: p.into(),
                    q: q.into(),
                    u: u.into(),
                })
            }

            DSA => {
                let x = MPI::parse("dsa_secret_len", "dsa_secret", php)?;

                Ok(mpi::SecretKeyMaterial::DSA {
                    x: x.into(),
                })
            }

            ElGamalEncrypt | ElGamalEncryptSign => {
                let x = MPI::parse("elgamal_secret_len", "elgamal_secret",
                                   php)?;

                Ok(mpi::SecretKeyMaterial::ElGamal {
                    x: x.into(),
                })
            }

            EdDSA => {
                Ok(mpi::SecretKeyMaterial::EdDSA {
                    scalar: MPI::parse("eddsa_secret_len", "eddsa_secret", php)?
                                .into()
                })
            }

            ECDSA => {
                Ok(mpi::SecretKeyMaterial::ECDSA {
                    scalar: MPI::parse("ecdsa_secret_len", "ecdsa_secret", php)?
                                .into()
                })
            }

            ECDH => {
                Ok(mpi::SecretKeyMaterial::ECDH {
                    scalar: MPI::parse("ecdh_secret_len", "ecdh_secret", php)?
                                .into()
                })
            }

            Unknown(_) | Private(_) => {
                let mut mpis = Vec::new();
                while let Ok(mpi) = MPI::parse("unknown_len",
                                               "unknown", php) {
                    mpis.push(mpi.into());
                }
                let rest = php.parse_bytes_eof("rest")?;

                Ok(mpi::SecretKeyMaterial::Unknown {
                    mpis: mpis.into_boxed_slice(),
                    rest: rest.into(),
                })
            }
        };
        let mpis = mpis?;

        if let Some(checksum) = checksum {
            use crate::serialize::{Marshal, MarshalInto};
            let good = match checksum {
                mpi::SecretKeyChecksum::SHA1 => {
                    // Read expected SHA1 hash of the MPIs.
                    let their_chksum = php.parse_bytes("checksum", 20)?;

                    // Compute SHA1 hash.
                    let mut hsh = HashAlgorithm::SHA1.context().unwrap();
                    mpis.serialize(&mut hsh)?;
                    let mut our_chksum = [0u8; 20];
                    let _ = hsh.digest(&mut our_chksum);

                    our_chksum == their_chksum[..]
                },

                mpi::SecretKeyChecksum::Sum16 => {
                    // Read expected sum of the MPIs.
                    let their_chksum = php.parse_bytes("checksum", 2)?;

                    // Compute sum.
                    let our_chksum = mpis.to_vec()?.iter()
                        .fold(0u16, |acc, v| acc.wrapping_add(*v as u16))
                        .to_be_bytes();

                    our_chksum == their_chksum[..]
                },
            };

            if good {
                Ok(mpis)
            } else {
                Err(Error::MalformedMPI("checksum wrong".to_string()).into())
            }
        } else {
            Ok(mpis)
        }
    }
}

impl mpi::Ciphertext {
    /// Parses a set of OpenPGP MPIs representing a ciphertext.
    ///
    /// Expects MPIs for a public key algorithm `algo`s ciphertext.
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    pub fn parse<R: Read + Send + Sync>(algo: PublicKeyAlgorithm, reader: R) -> Result<Self>
    {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        let mut php = PacketHeaderParser::new_naked(bio);
        Self::_parse(algo, &mut php)
    }

    /// Parses a set of OpenPGP MPIs representing a ciphertext.
    ///
    /// Expects MPIs for a public key algorithm `algo`s ciphertext.
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    pub(crate) fn _parse<'a, T: 'a + BufferedReader<Cookie>>(
        algo: PublicKeyAlgorithm,
        php: &mut PacketHeaderParser<T>)
        -> Result<Self> {
        use crate::PublicKeyAlgorithm::*;

        #[allow(deprecated)]
        match algo {
            RSAEncryptSign | RSAEncrypt => {
                let c = MPI::parse("rsa_ciphertxt_len", "rsa_ciphertxt",
                                   php)?;

                Ok(mpi::Ciphertext::RSA {
                    c,
                })
            }

            ElGamalEncrypt | ElGamalEncryptSign => {
                let e = MPI::parse("elgamal_e_len", "elgamal_e", php)?;
                let c = MPI::parse("elgamal_c_len", "elgamal_c", php)?;

                Ok(mpi::Ciphertext::ElGamal {
                    e,
                    c,
                })
            }

            ECDH => {
                let e = MPI::parse("ecdh_e_len", "ecdh_e", php)?;
                let key_len = php.parse_u8("ecdh_esk_len")? as usize;
                let key = Vec::from(&php.parse_bytes("ecdh_esk", key_len)?
                                    [..key_len]);

                Ok(mpi::Ciphertext::ECDH {
                    e, key: key.into_boxed_slice()
                })
            }

            Unknown(_) | Private(_) => {
                let mut mpis = Vec::new();
                while let Ok(mpi) = MPI::parse("unknown_len",
                                               "unknown", php) {
                    mpis.push(mpi);
                }
                let rest = php.parse_bytes_eof("rest")?;

                Ok(mpi::Ciphertext::Unknown {
                    mpis: mpis.into_boxed_slice(),
                    rest: rest.into_boxed_slice(),
                })
            }

            RSASign | DSA | EdDSA | ECDSA => Err(Error::InvalidArgument(
                format!("not an encryption algorithm: {:?}", algo)).into()),
        }
    }
}

impl mpi::Signature {
    /// Parses a set of OpenPGP MPIs representing a signature.
    ///
    /// Expects MPIs for a public key algorithm `algo`s signature.
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    pub fn parse<R: Read + Send + Sync>(algo: PublicKeyAlgorithm, reader: R) -> Result<Self>
    {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        let mut php = PacketHeaderParser::new_naked(bio);
        Self::_parse(algo, &mut php)
    }

    /// Parses a set of OpenPGP MPIs representing a signature.
    ///
    /// Expects MPIs for a public key algorithm `algo`s signature.
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    pub(crate) fn _parse<'a, T: 'a + BufferedReader<Cookie>>(
        algo: PublicKeyAlgorithm,
        php: &mut PacketHeaderParser<T>)
        -> Result<Self> {
        use crate::PublicKeyAlgorithm::*;

        #[allow(deprecated)]
        match algo {
            RSAEncryptSign | RSASign => {
                let s = MPI::parse("rsa_signature_len", "rsa_signature", php)?;

                Ok(mpi::Signature::RSA {
                    s,
                })
            }

            DSA => {
                let r = MPI::parse("dsa_sig_r_len", "dsa_sig_r",
                                   php)?;
                let s = MPI::parse("dsa_sig_s_len", "dsa_sig_s",
                                   php)?;

                Ok(mpi::Signature::DSA {
                    r,
                    s,
                })
            }

            ElGamalEncryptSign => {
                let r = MPI::parse("elgamal_sig_r_len",
                                   "elgamal_sig_r", php)?;
                let s = MPI::parse("elgamal_sig_s_len",
                                   "elgamal_sig_s", php)?;

                Ok(mpi::Signature::ElGamal {
                    r,
                    s,
                })
            }

            EdDSA => {
                let r = MPI::parse("eddsa_sig_r_len", "eddsa_sig_r",
                                   php)?;
                let s = MPI::parse("eddsa_sig_s_len", "eddsa_sig_s",
                                   php)?;

                Ok(mpi::Signature::EdDSA {
                    r,
                    s,
                })
            }

            ECDSA => {
                let r = MPI::parse("ecdsa_sig_r_len", "ecdsa_sig_r",
                                   php)?;
                let s = MPI::parse("ecdsa_sig_s_len", "ecdsa_sig_s",
                                   php)?;

                Ok(mpi::Signature::ECDSA {
                    r,
                    s,
                })
            }

            Unknown(_) | Private(_) => {
                let mut mpis = Vec::new();
                while let Ok(mpi) = MPI::parse("unknown_len",
                                               "unknown", php) {
                    mpis.push(mpi);
                }
                let rest = php.parse_bytes_eof("rest")?;

                Ok(mpi::Signature::Unknown {
                    mpis: mpis.into_boxed_slice(),
                    rest: rest.into_boxed_slice(),
                })
            }

            RSAEncrypt | ElGamalEncrypt | ECDH => Err(Error::InvalidArgument(
                format!("not a signature algorithm: {:?}", algo)).into()),
        }
    }
}

#[test]
fn mpis_parse_test() {
    use std::io::Cursor;
    use super::Parse;
    use crate::PublicKeyAlgorithm::*;
    use crate::serialize::MarshalInto;

    // Dummy RSA public key.
    {
        let buf = Cursor::new("\x00\x01\x01\x00\x02\x02");
        let mpis = mpi::PublicKey::parse(RSAEncryptSign, buf).unwrap();

        //assert_eq!(mpis.serialized_len(), 6);
        match &mpis {
            &mpi::PublicKey::RSA{ ref n, ref e } => {
                assert_eq!(n.bits(), 1);
                assert_eq!(n.value()[0], 1);
                assert_eq!(n.value().len(), 1);
                assert_eq!(e.bits(), 2);
                assert_eq!(e.value()[0], 2);
                assert_eq!(e.value().len(), 1);
            }

            _ => assert!(false),
        }
    }

    // The number 2.
    {
        let buf = Cursor::new("\x00\x02\x02");
        let mpis = mpi::Ciphertext::parse(RSAEncryptSign, buf).unwrap();

        assert_eq!(mpis.serialized_len(), 3);
    }

    // The number 511.
    let mpi = MPI::from_bytes(b"\x00\x09\x01\xff").unwrap();
    assert_eq!(mpi.value().len(), 2);
    assert_eq!(mpi.bits(), 9);
    assert_eq!(mpi.value()[0], 1);
    assert_eq!(mpi.value()[1], 0xff);

    // The number 1, incorrectly encoded (the length should be 1,
    // not 2).
    assert!(MPI::from_bytes(b"\x00\x02\x01").is_err());
}
