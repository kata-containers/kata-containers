//! Hold the implementation of [`Signer`] and [`Decryptor`] for [`KeyPair`].
//!
//! [`Signer`]: super::super::asymmetric::Signer
//! [`Decryptor`]: super::super::asymmetric::Decryptor
//! [`KeyPair`]: super::super::asymmetric::KeyPair

use nettle::{curve25519, ecc, ecdh, ecdsa, ed25519, dsa, rsa, random::Yarrow};

use crate::{Error, Result};

use crate::packet::{key, Key};
use crate::crypto::asymmetric::{KeyPair, Decryptor, Signer};
use crate::crypto::mpi::{self, MPI, PublicKey};
use crate::crypto::SessionKey;
use crate::types::{Curve, HashAlgorithm};

impl Signer for KeyPair {
    fn public(&self) -> &Key<key::PublicParts, key::UnspecifiedRole> {
        KeyPair::public(self)
    }

    fn sign(&mut self, hash_algo: HashAlgorithm, digest: &[u8])
            -> Result<mpi::Signature>
    {
        use crate::PublicKeyAlgorithm::*;

        let mut rng = Yarrow::default();

        self.secret().map(|secret| {
            #[allow(deprecated)]
            match (self.public().pk_algo(), self.public().mpis(), secret)
        {
            (RSASign,
             &PublicKey::RSA { ref e, ref n },
             &mpi::SecretKeyMaterial::RSA { ref p, ref q, ref d, .. }) |
            (RSAEncryptSign,
             &PublicKey::RSA { ref e, ref n },
             &mpi::SecretKeyMaterial::RSA { ref p, ref q, ref d, .. }) => {
                let public = rsa::PublicKey::new(n.value(), e.value())?;
                let secret = rsa::PrivateKey::new(d.value(), p.value(),
                                                  q.value(), Option::None)?;

                // The signature has the length of the modulus.
                let mut sig = vec![0u8; n.value().len()];

                // As described in [Section 5.2.2 and 5.2.3 of RFC 4880],
                // to verify the signature, we need to encode the
                // signature data in a PKCS1-v1.5 packet.
                //
                //   [Section 5.2.2 and 5.2.3 of RFC 4880]:
                //   https://tools.ietf.org/html/rfc4880#section-5.2.2
                rsa::sign_digest_pkcs1(&public, &secret, digest,
                                       hash_algo.oid()?,
                                       &mut rng, &mut sig)?;

                Ok(mpi::Signature::RSA {
                    s: MPI::new(&sig),
                })
            },

            (DSA,
             &PublicKey::DSA { ref p, ref q, ref g, .. },
             &mpi::SecretKeyMaterial::DSA { ref x }) => {
                let params = dsa::Params::new(p.value(), q.value(), g.value());
                let secret = dsa::PrivateKey::new(x.value());

                let sig = dsa::sign(&params, &secret, digest, &mut rng)?;

                Ok(mpi::Signature::DSA {
                    r: MPI::new(&sig.r()),
                    s: MPI::new(&sig.s()),
                })
            },

            (EdDSA,
             &PublicKey::EdDSA { ref curve, ref q },
             &mpi::SecretKeyMaterial::EdDSA { ref scalar }) => match curve {
                Curve::Ed25519 => {
                    let public = q.decode_point(&Curve::Ed25519)?.0;

                    let mut sig = vec![0; ed25519::ED25519_SIGNATURE_SIZE];

                    // Nettle expects the private key to be exactly
                    // ED25519_KEY_SIZE bytes long but OpenPGP allows leading
                    // zeros to be stripped.
                    // Padding has to be unconditional; otherwise we have a
                    // secret-dependent branch.
                    let sec = scalar.value_padded(ed25519::ED25519_KEY_SIZE);

                    ed25519::sign(public, &sec[..], digest, &mut sig)?;

                    Ok(mpi::Signature::EdDSA {
                        r: MPI::new(&sig[..ed25519::ED25519_KEY_SIZE]),
                        s: MPI::new(&sig[ed25519::ED25519_KEY_SIZE..]),
                    })
                },
                _ => Err(
                    Error::UnsupportedEllipticCurve(curve.clone()).into()),
            },

            (ECDSA,
             &PublicKey::ECDSA { ref curve, .. },
             &mpi::SecretKeyMaterial::ECDSA { ref scalar }) => {
                let secret = match curve {
                    Curve::NistP256 =>
                        ecc::Scalar::new::<ecc::Secp256r1>(
                            scalar.value())?,
                    Curve::NistP384 =>
                        ecc::Scalar::new::<ecc::Secp384r1>(
                            scalar.value())?,
                    Curve::NistP521 =>
                        ecc::Scalar::new::<ecc::Secp521r1>(
                            scalar.value())?,
                    _ =>
                        return Err(
                            Error::UnsupportedEllipticCurve(curve.clone())
                                .into()),
                };

                let sig = ecdsa::sign(&secret, digest, &mut rng);

                Ok(mpi::Signature::ECDSA {
                    r: MPI::new(&sig.r()),
                    s: MPI::new(&sig.s()),
                })
            },

            (pk_algo, _, _) => Err(Error::InvalidOperation(format!(
                "unsupported combination of algorithm {:?}, key {:?}, \
                 and secret key {:?}",
                pk_algo, self.public(), self.secret())).into()),
        }})
    }
}

impl Decryptor for KeyPair {
    fn public(&self) -> &Key<key::PublicParts, key::UnspecifiedRole> {
        KeyPair::public(self)
    }

    fn decrypt(&mut self, ciphertext: &mpi::Ciphertext,
               plaintext_len: Option<usize>)
               -> Result<SessionKey>
    {
        use crate::PublicKeyAlgorithm::*;

        self.secret().map(
            |secret| Ok(match (self.public().mpis(), secret, ciphertext)
        {
            (PublicKey::RSA{ ref e, ref n },
             mpi::SecretKeyMaterial::RSA{ ref p, ref q, ref d, .. },
             mpi::Ciphertext::RSA{ ref c }) => {
                let public = rsa::PublicKey::new(n.value(), e.value())?;
                let secret = rsa::PrivateKey::new(d.value(), p.value(),
                                                  q.value(), Option::None)?;
                let mut rand = Yarrow::default();
                if let Some(l) = plaintext_len {
                    let mut plaintext: SessionKey = vec![0; l].into();
                    rsa::decrypt_pkcs1(&public, &secret, &mut rand,
                                       c.value(), plaintext.as_mut())?;
                    plaintext
                } else {
                    rsa::decrypt_pkcs1_insecure(&public, &secret,
                                                &mut rand, c.value())?
                    .into()
                }
            }

            (PublicKey::ElGamal{ .. },
             mpi::SecretKeyMaterial::ElGamal{ .. },
             mpi::Ciphertext::ElGamal{ .. }) =>
                return Err(
                    Error::UnsupportedPublicKeyAlgorithm(ElGamalEncrypt).into()),

            (PublicKey::ECDH{ .. },
             mpi::SecretKeyMaterial::ECDH { .. },
             mpi::Ciphertext::ECDH { .. }) =>
                crate::crypto::ecdh::decrypt(self.public(), secret, ciphertext)?,

            (public, secret, ciphertext) =>
                return Err(Error::InvalidOperation(format!(
                    "unsupported combination of key pair {:?}/{:?} \
                     and ciphertext {:?}",
                    public, secret, ciphertext)).into()),
        }))
    }
}


impl<P: key::KeyParts, R: key::KeyRole> Key<P, R> {
    /// Encrypts the given data with this key.
    pub fn encrypt(&self, data: &SessionKey) -> Result<mpi::Ciphertext> {
        use crate::PublicKeyAlgorithm::*;

        #[allow(deprecated)]
        match self.pk_algo() {
            RSAEncryptSign | RSAEncrypt => {
                // Extract the public recipient.
                match self.mpis() {
                    mpi::PublicKey::RSA { e, n } => {
                        // The ciphertext has the length of the modulus.
                        let ciphertext_len = n.value().len();
                        if data.len() + 11 > ciphertext_len {
                            return Err(Error::InvalidArgument(
                                "Plaintext data too large".into()).into());
                        }

                        let mut esk = vec![0u8; ciphertext_len];
                        let mut rng = Yarrow::default();
                        let pk = rsa::PublicKey::new(n.value(), e.value())?;
                        rsa::encrypt_pkcs1(&pk, &mut rng, data,
                                           &mut esk)?;
                        Ok(mpi::Ciphertext::RSA {
                            c: MPI::new(&esk),
                        })
                    },
                    pk => {
                        Err(Error::MalformedPacket(
                            format!(
                                "Key: Expected RSA public key, got {:?}",
                                pk)).into())
                    },
                }
            },
            ECDH => crate::crypto::ecdh::encrypt(self.parts_as_public(),
                                                 data),
            algo => Err(Error::UnsupportedPublicKeyAlgorithm(algo).into()),
        }
    }

    /// Verifies the given signature.
    pub fn verify(&self, sig: &mpi::Signature, hash_algo: HashAlgorithm,
                  digest: &[u8]) -> Result<()>
    {
        use crate::crypto::mpi::Signature;

        fn bad(e: impl ToString) -> anyhow::Error {
            Error::BadSignature(e.to_string()).into()
        }

        let ok = match (self.mpis(), sig) {
            (PublicKey::RSA { e, n }, Signature::RSA { s }) => {
                let key = rsa::PublicKey::new(n.value(), e.value())?;

                // As described in [Section 5.2.2 and 5.2.3 of RFC 4880],
                // to verify the signature, we need to encode the
                // signature data in a PKCS1-v1.5 packet.
                //
                //   [Section 5.2.2 and 5.2.3 of RFC 4880]:
                //   https://tools.ietf.org/html/rfc4880#section-5.2.2
                rsa::verify_digest_pkcs1(&key, digest, hash_algo.oid()?,
                                         s.value())?
            },
            (PublicKey::DSA { y, p, q, g }, Signature::DSA { s, r }) => {
                let key = dsa::PublicKey::new(y.value());
                let params = dsa::Params::new(p.value(), q.value(), g.value());
                let signature = dsa::Signature::new(r.value(), s.value());

                dsa::verify(&params, &key, digest, &signature)
            },
            (PublicKey::EdDSA { curve, q }, Signature::EdDSA { r, s }) =>
              match curve {
                Curve::Ed25519 => {
                    if q.value().get(0).map(|&b| b != 0x40).unwrap_or(true) {
                        return Err(Error::MalformedPacket(
                            "Invalid point encoding".into()).into());
                    }

                    // OpenPGP encodes R and S separately, but our
                    // cryptographic library expects them to be
                    // concatenated.
                    let mut signature =
                        Vec::with_capacity(ed25519::ED25519_SIGNATURE_SIZE);

                    // We need to zero-pad them at the front, because
                    // the MPI encoding drops leading zero bytes.
                    let half = ed25519::ED25519_SIGNATURE_SIZE / 2;
                    signature.extend_from_slice(
                        &r.value_padded(half).map_err(bad)?);
                    signature.extend_from_slice(
                        &s.value_padded(half).map_err(bad)?);

                    // Let's see if we got it right.
                    assert_eq!(signature.len(),
                               ed25519::ED25519_SIGNATURE_SIZE);

                    ed25519::verify(&q.value()[1..], digest, &signature)?
                },
                _ => return
                    Err(Error::UnsupportedEllipticCurve(curve.clone()).into()),
            },
            (PublicKey::ECDSA { curve, q }, Signature::ECDSA { s, r }) =>
            {
                let (x, y) = q.decode_point(curve)?;
                let key = match curve {
                    Curve::NistP256 => ecc::Point::new::<ecc::Secp256r1>(x, y)?,
                    Curve::NistP384 => ecc::Point::new::<ecc::Secp384r1>(x, y)?,
                    Curve::NistP521 => ecc::Point::new::<ecc::Secp521r1>(x, y)?,
                    _ => return Err(
                        Error::UnsupportedEllipticCurve(curve.clone()).into()),
                };

                let signature = dsa::Signature::new(r.value(), s.value());
                ecdsa::verify(&key, digest, &signature)
            },
            _ => return Err(Error::MalformedPacket(format!(
                "unsupported combination of key {} and signature {:?}.",
                self.pk_algo(), sig)).into()),
        };

        if ok {
            Ok(())
        } else {
            Err(Error::ManipulatedMessage.into())
        }
    }
}

use std::time::SystemTime;
use crate::crypto::mem::Protected;
use crate::packet::key::{Key4, SecretParts};
use crate::types::{PublicKeyAlgorithm, SymmetricAlgorithm};

impl<R> Key4<SecretParts, R>
    where R: key::KeyRole,
{

    /// Creates a new OpenPGP secret key packet for an existing X25519 key.
    ///
    /// The ECDH key will use hash algorithm `hash` and symmetric
    /// algorithm `sym`.  If one or both are `None` secure defaults
    /// will be used.  The key will have it's creation date set to
    /// `ctime` or the current time if `None` is given.
    pub fn import_secret_cv25519<H, S, T>(private_key: &[u8],
                                          hash: H, sym: S, ctime: T)
        -> Result<Self> where H: Into<Option<HashAlgorithm>>,
                              S: Into<Option<SymmetricAlgorithm>>,
                              T: Into<Option<SystemTime>>
    {
        let mut public_key = [0; curve25519::CURVE25519_SIZE];
        curve25519::mul_g(&mut public_key, private_key).unwrap();

        let mut private_key = Vec::from(private_key);
        private_key.reverse();

        use crate::crypto::ecdh;
        Self::with_secret(
            ctime.into().unwrap_or_else(crate::now),
            PublicKeyAlgorithm::ECDH,
            mpi::PublicKey::ECDH {
                curve: Curve::Cv25519,
                hash: hash.into().unwrap_or_else(
                    || ecdh::default_ecdh_kdf_hash(&Curve::Cv25519)),
                sym: sym.into().unwrap_or_else(
                    || ecdh::default_ecdh_kek_cipher(&Curve::Cv25519)),
                q: MPI::new_compressed_point(&public_key),
            },
            mpi::SecretKeyMaterial::ECDH {
                scalar: private_key.into(),
            }.into())
    }

    /// Creates a new OpenPGP secret key packet for an existing Ed25519 key.
    ///
    /// The ECDH key will use hash algorithm `hash` and symmetric
    /// algorithm `sym`.  If one or both are `None` secure defaults
    /// will be used.  The key will have it's creation date set to
    /// `ctime` or the current time if `None` is given.
    pub fn import_secret_ed25519<T>(private_key: &[u8], ctime: T)
        -> Result<Self> where T: Into<Option<SystemTime>>
    {
        let mut public_key = [0; ed25519::ED25519_KEY_SIZE];
        ed25519::public_key(&mut public_key, private_key).unwrap();

        Self::with_secret(
            ctime.into().unwrap_or_else(crate::now),
            PublicKeyAlgorithm::EdDSA,
            mpi::PublicKey::EdDSA {
                curve: Curve::Ed25519,
                q: MPI::new_compressed_point(&public_key),
            },
            mpi::SecretKeyMaterial::EdDSA {
                scalar: mpi::MPI::new(private_key).into(),
            }.into())
    }

    /// Creates a new OpenPGP public key packet for an existing RSA key.
    ///
    /// The RSA key will use public exponent `e` and modulo `n`. The key will
    /// have it's creation date set to `ctime` or the current time if `None`
    /// is given.
    #[allow(clippy::many_single_char_names)]
    pub fn import_secret_rsa<T>(d: &[u8], p: &[u8], q: &[u8], ctime: T)
        -> Result<Self> where T: Into<Option<SystemTime>>
    {
        let sec = rsa::PrivateKey::new(d, p, q, None)?;
        let key = sec.public_key()?;
        let (a, b, c) = sec.as_rfc4880();

        Self::with_secret(
            ctime.into().unwrap_or_else(crate::now),
            PublicKeyAlgorithm::RSAEncryptSign,
            mpi::PublicKey::RSA {
                e: mpi::MPI::new(&key.e()[..]),
                n: mpi::MPI::new(&key.n()[..]),
            },
            mpi::SecretKeyMaterial::RSA {
                d: mpi::MPI::new(d).into(),
                p: mpi::MPI::new(&a[..]).into(),
                q: mpi::MPI::new(&b[..]).into(),
                u: mpi::MPI::new(&c[..]).into(),
            }.into())
    }

    /// Generates a new RSA key with a public modulos of size `bits`.
    pub fn generate_rsa(bits: usize) -> Result<Self> {
        let mut rng = Yarrow::default();

        let (public, private) = rsa::generate_keypair(&mut rng, bits as u32)?;
        let (p, q, u) = private.as_rfc4880();
        let public_mpis = PublicKey::RSA {
            e: MPI::new(&*public.e()),
            n: MPI::new(&*public.n()),
        };
        let private_mpis = mpi::SecretKeyMaterial::RSA {
            d: MPI::new(&*private.d()).into(),
            p: MPI::new(&*p).into(),
            q: MPI::new(&*q).into(),
            u: MPI::new(&*u).into(),
        };

        Self::with_secret(
            crate::now(),
            PublicKeyAlgorithm::RSAEncryptSign,
            public_mpis,
            private_mpis.into())
    }

    /// Generates a new ECC key over `curve`.
    ///
    /// If `for_signing` is false a ECDH key, if it's true either a
    /// EdDSA or ECDSA key is generated.  Giving `for_signing == true` and
    /// `curve == Cv25519` will produce an error. Likewise
    /// `for_signing == false` and `curve == Ed25519` will produce an error.
    pub fn generate_ecc(for_signing: bool, curve: Curve) -> Result<Self> {
        use crate::PublicKeyAlgorithm::*;

        let mut rng = Yarrow::default();
        let hash = crate::crypto::ecdh::default_ecdh_kdf_hash(&curve);
        let sym = crate::crypto::ecdh::default_ecdh_kek_cipher(&curve);

        let (mpis, secret, pk_algo) = match (curve.clone(), for_signing) {
            (Curve::Ed25519, true) => {
                let mut public = [0; ed25519::ED25519_KEY_SIZE];
                let private: Protected =
                    ed25519::private_key(&mut rng).into();
                ed25519::public_key(&mut public, &private)?;

                let public_mpis = PublicKey::EdDSA {
                    curve: Curve::Ed25519,
                    q: MPI::new_compressed_point(&public),
                };
                let private_mpis = mpi::SecretKeyMaterial::EdDSA {
                    scalar: private.into(),
                };
                let sec = private_mpis.into();

                (public_mpis, sec, EdDSA)
            }

            (Curve::Cv25519, false) => {
                let mut public = [0; curve25519::CURVE25519_SIZE];
                let mut private: Protected =
                    curve25519::private_key(&mut rng).into();
                curve25519::mul_g(&mut public, &private)?;

                // Reverse the scalar.  See
                // https://lists.gnupg.org/pipermail/gnupg-devel/2018-February/033437.html.
                private.reverse();

                let public_mpis = PublicKey::ECDH {
                    curve: Curve::Cv25519,
                    q: MPI::new_compressed_point(&public),
                    hash,
                    sym,
                };
                let private_mpis = mpi::SecretKeyMaterial::ECDH {
                    scalar: private.into(),
                };
                let sec = private_mpis.into();

                (public_mpis, sec, ECDH)
            }

            (Curve::NistP256, true)  | (Curve::NistP384, true)
            | (Curve::NistP521, true) => {
                let (public, private, field_sz) = match curve {
                    Curve::NistP256 => {
                        let (pu, sec) =
                            ecdsa::generate_keypair::<ecc::Secp256r1, _>(&mut rng)?;
                        (pu, sec, 256)
                    }
                    Curve::NistP384 => {
                        let (pu, sec) =
                            ecdsa::generate_keypair::<ecc::Secp384r1, _>(&mut rng)?;
                        (pu, sec, 384)
                    }
                    Curve::NistP521 => {
                        let (pu, sec) =
                            ecdsa::generate_keypair::<ecc::Secp521r1, _>(&mut rng)?;
                        (pu, sec, 521)
                    }
                    _ => unreachable!(),
                };
                let (pub_x, pub_y) = public.as_bytes();
                let public_mpis =  mpi::PublicKey::ECDSA{
                    curve,
                    q: MPI::new_point(&pub_x, &pub_y, field_sz),
                };
                let private_mpis = mpi::SecretKeyMaterial::ECDSA{
                    scalar: MPI::new(&private.as_bytes()).into(),
                };
                let sec = private_mpis.into();

                (public_mpis, sec, ECDSA)
            }

            (Curve::NistP256, false)  | (Curve::NistP384, false)
            | (Curve::NistP521, false) => {
                    let (private, field_sz) = match curve {
                        Curve::NistP256 => {
                            let pv =
                                ecc::Scalar::new_random::<ecc::Secp256r1, _>(&mut rng);

                            (pv, 256)
                        }
                        Curve::NistP384 => {
                            let pv =
                                ecc::Scalar::new_random::<ecc::Secp384r1, _>(&mut rng);

                            (pv, 384)
                        }
                        Curve::NistP521 => {
                            let pv =
                                ecc::Scalar::new_random::<ecc::Secp521r1, _>(&mut rng);

                            (pv, 521)
                        }
                        _ => unreachable!(),
                    };
                    let public = ecdh::point_mul_g(&private);
                    let (pub_x, pub_y) = public.as_bytes();
                    let public_mpis = mpi::PublicKey::ECDH{
                        curve,
                        q: MPI::new_point(&pub_x, &pub_y, field_sz),
                        hash,
                        sym,
                    };
                    let private_mpis = mpi::SecretKeyMaterial::ECDH{
                        scalar: MPI::new(&private.as_bytes()).into(),
                    };
                    let sec = private_mpis.into();

                    (public_mpis, sec, ECDH)
                }

            (cv, _) => {
                return Err(Error::UnsupportedEllipticCurve(cv).into());
            }
        };

        Self::with_secret(
            crate::now(),
            pk_algo,
            mpis,
            secret)
    }
}
