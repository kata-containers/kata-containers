// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! BL-2 — multi-algorithm public-key verification for fragment signatures.
//!
//! A single verifier used by both the did:x509 certificate path (leaf COSE signature +
//! chain-link certificate signatures) and the transparency trust list (receipt / signed
//! tree-head signatures). Supported algorithms (pure-Rust RustCrypto, no Go):
//!
//! | COSE alg | id  | key         | hash    |
//! |----------|-----|-------------|---------|
//! | EdDSA    | -8  | Ed25519     | (n/a)   |
//! | ES256    | -7  | EC P-256    | SHA-256 |
//! | ES384    | -35 | EC P-384    | SHA-384 |
//! | PS256    | -37 | RSA (PSS)   | SHA-256 |
//! | RS256    |-257 | RSA (PKCS1) | SHA-256 |
//!
//! For X.509 chain links the certificate `signatureAlgorithm` OID selects the scheme:
//! ecdsa-with-SHA256/384, sha256/384-WithRSAEncryption (PKCS#1 v1.5). RSA-PSS in certificates
//! is uncommon and intentionally not accepted for chain links (fail-closed).

use const_oid::ObjectIdentifier;
use ed25519_dalek::Verifier as _;
use std::convert::TryFrom;

/// COSE signature algorithms we accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoseAlg {
    EdDsa,
    Es256,
    Es384,
    Ps256,
    Rs256,
}

impl CoseAlg {
    /// Map a COSE algorithm integer (RFC 9053 / IANA COSE registry) to a supported scheme.
    pub fn from_i64(v: i64) -> Option<Self> {
        match v {
            -8 => Some(CoseAlg::EdDsa),
            -7 => Some(CoseAlg::Es256),
            -35 => Some(CoseAlg::Es384),
            -37 => Some(CoseAlg::Ps256),
            -257 => Some(CoseAlg::Rs256),
            _ => None,
        }
    }
}

// Certificate signatureAlgorithm OIDs.
const ECDSA_WITH_SHA256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.2");
const ECDSA_WITH_SHA384: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.3");
const SHA256_WITH_RSA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.11");
const SHA384_WITH_RSA: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.12");
// SubjectPublicKeyInfo algorithm OID for RSA.
const RSA_ENCRYPTION: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");

/// A parsed public key of one of the supported algorithms.
#[derive(Clone)]
pub enum PublicKey {
    Ed25519(ed25519_dalek::VerifyingKey),
    P256(p256::ecdsa::VerifyingKey),
    P384(p384::ecdsa::VerifyingKey),
    Rsa(rsa::RsaPublicKey),
}

impl PublicKey {
    /// Parse a public key from a raw 32-byte Ed25519 key.
    pub fn from_ed25519_bytes(bytes: &[u8; 32]) -> Option<Self> {
        ed25519_dalek::VerifyingKey::from_bytes(bytes)
            .ok()
            .map(PublicKey::Ed25519)
    }

    /// Parse a public key from a DER-encoded SubjectPublicKeyInfo (as found in a certificate
    /// or a configured ledger key). Tries EC P-256, EC P-384, then RSA.
    pub fn from_spki_der(der: &[u8]) -> Option<Self> {
        use spki::DecodePublicKey;
        if let Ok(k) = p256::ecdsa::VerifyingKey::from_public_key_der(der) {
            return Some(PublicKey::P256(k));
        }
        if let Ok(k) = p384::ecdsa::VerifyingKey::from_public_key_der(der) {
            return Some(PublicKey::P384(k));
        }
        // RSA: pull the PKCS#1 RSAPublicKey out of the SPKI and parse it.
        if let Ok(spki) = spki::SubjectPublicKeyInfoRef::try_from(der) {
            if spki.algorithm.oid == RSA_ENCRYPTION {
                if let Some(pk_der) = spki.subject_public_key.as_bytes() {
                    use rsa::pkcs1::DecodeRsaPublicKey;
                    if let Ok(k) = rsa::RsaPublicKey::from_pkcs1_der(pk_der) {
                        return Some(PublicKey::Rsa(k));
                    }
                }
            }
        }
        None
    }

    /// Verify a COSE detached signature (`sig`) over `tbs` under `alg`. `sig` is in the COSE
    /// wire form: fixed-width `r||s` for ECDSA, raw modulus-width bytes for RSA, 64 bytes for
    /// EdDSA. Returns `Ok(())` iff the signature is valid and `alg` matches this key type.
    pub fn verify_cose(&self, alg: CoseAlg, tbs: &[u8], sig: &[u8]) -> Result<(), ()> {
        match (self, alg) {
            (PublicKey::Ed25519(k), CoseAlg::EdDsa) => {
                let s = ed25519_dalek::Signature::from_slice(sig).map_err(|_| ())?;
                k.verify(tbs, &s).map_err(|_| ())
            }
            (PublicKey::P256(k), CoseAlg::Es256) => {
                let s = p256::ecdsa::Signature::from_slice(sig).map_err(|_| ())?;
                k.verify(tbs, &s).map_err(|_| ())
            }
            (PublicKey::P384(k), CoseAlg::Es384) => {
                let s = p384::ecdsa::Signature::from_slice(sig).map_err(|_| ())?;
                k.verify(tbs, &s).map_err(|_| ())
            }
            (PublicKey::Rsa(k), CoseAlg::Ps256) => {
                let vk = rsa::pss::VerifyingKey::<sha2::Sha256>::new(k.clone());
                let s = rsa::pss::Signature::try_from(sig).map_err(|_| ())?;
                vk.verify(tbs, &s).map_err(|_| ())
            }
            (PublicKey::Rsa(k), CoseAlg::Rs256) => {
                let vk = rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(k.clone());
                let s = rsa::pkcs1v15::Signature::try_from(sig).map_err(|_| ())?;
                vk.verify(tbs, &s).map_err(|_| ())
            }
            // Algorithm does not match the key type ⇒ reject (no cross-alg confusion).
            _ => Err(()),
        }
    }

    /// Verify an X.509 certificate signature: `sig_der` is the certificate's `signatureValue`
    /// (DER ECDSA-Sig-Value for ECDSA, raw for RSA) and `sig_alg_oid` its `signatureAlgorithm`.
    /// This key is the *issuer* key. Returns `Ok(())` iff valid and the scheme is supported and
    /// consistent with this key type.
    pub fn verify_cert_sig(
        &self,
        sig_alg_oid: &ObjectIdentifier,
        tbs: &[u8],
        sig_der: &[u8],
    ) -> Result<(), ()> {
        match (self, *sig_alg_oid) {
            (PublicKey::P256(k), oid) if oid == ECDSA_WITH_SHA256 => {
                let s = p256::ecdsa::DerSignature::try_from(sig_der).map_err(|_| ())?;
                k.verify(tbs, &s).map_err(|_| ())
            }
            (PublicKey::P384(k), oid) if oid == ECDSA_WITH_SHA384 => {
                let s = p384::ecdsa::DerSignature::try_from(sig_der).map_err(|_| ())?;
                k.verify(tbs, &s).map_err(|_| ())
            }
            (PublicKey::Rsa(k), oid) if oid == SHA256_WITH_RSA => {
                let vk = rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(k.clone());
                let s = rsa::pkcs1v15::Signature::try_from(sig_der).map_err(|_| ())?;
                vk.verify(tbs, &s).map_err(|_| ())
            }
            (PublicKey::Rsa(k), oid) if oid == SHA384_WITH_RSA => {
                let vk = rsa::pkcs1v15::VerifyingKey::<sha2::Sha384>::new(k.clone());
                let s = rsa::pkcs1v15::Signature::try_from(sig_der).map_err(|_| ())?;
                vk.verify(tbs, &s).map_err(|_| ())
            }
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cose_alg_mapping() {
        assert_eq!(CoseAlg::from_i64(-8), Some(CoseAlg::EdDsa));
        assert_eq!(CoseAlg::from_i64(-7), Some(CoseAlg::Es256));
        assert_eq!(CoseAlg::from_i64(-35), Some(CoseAlg::Es384));
        assert_eq!(CoseAlg::from_i64(-37), Some(CoseAlg::Ps256));
        assert_eq!(CoseAlg::from_i64(-257), Some(CoseAlg::Rs256));
        assert_eq!(CoseAlg::from_i64(-999), None);
    }

    #[test]
    fn ed25519_roundtrip_and_alg_mismatch() {
        use ed25519_dalek::{Signer, SigningKey};
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk = PublicKey::from_ed25519_bytes(&sk.verifying_key().to_bytes()).unwrap();
        let msg = b"hello";
        let sig = sk.sign(msg).to_bytes().to_vec();
        assert!(pk.verify_cose(CoseAlg::EdDsa, msg, &sig).is_ok());
        // Wrong algorithm for an Ed25519 key is rejected (no cross-alg confusion).
        assert!(pk.verify_cose(CoseAlg::Es256, msg, &sig).is_err());
        // Tampered message rejected.
        assert!(pk.verify_cose(CoseAlg::EdDsa, b"hell0", &sig).is_err());
    }
}
