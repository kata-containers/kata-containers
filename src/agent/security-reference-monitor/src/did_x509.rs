// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-1d — `did:x509` issuer identity for policy fragments.
//!
//! Instead of pinning a single issuer public key, an issuer may be proven by an **X.509
//! certificate chain** carried in the COSE_Sign1 `x5chain` header (COSE header label 33).
//! Trust is anchored on a **CA certificate fingerprint plus a `did:x509` policy** (required
//! subject CN / EKU / SAN over the leaf), *not* on a leaf key — so leaf **rotation** under
//! the same CA and policy is accepted with no configuration change, and **revocation** is a
//! measured fingerprint list.
//!
//! Verification is fully self-contained (no network, no Go dependency): X.509 parsing via
//! `x509-cert`, chain-link and leaf signatures via `p256` (ECDSA P-256 / SHA-256), which is
//! the common code-signing algorithm. The design mirrors runhcs/OpenGCS
//! (`didx509resolver.Resolve` over the `x5chain`) while keeping the raw-Ed25519 issuer path
//! untouched — the two identity models coexist and there is no downgrade path.

use crate::FragmentError;
use const_oid::db::rfc5280::{
    ID_CE_BASIC_CONSTRAINTS, ID_CE_EXT_KEY_USAGE, ID_CE_SUBJECT_ALT_NAME,
};
use const_oid::db::rfc5912::ID_EC_PUBLIC_KEY;
use const_oid::ObjectIdentifier;
use der::{Decode, Encode};
use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use sha2::{Digest, Sha256};
use spki::DecodePublicKey;
use std::collections::{HashMap, HashSet};
use x509_cert::Certificate;

/// COSE header parameter label for `x5chain` (RFC 9360).
const COSE_HEADER_X5CHAIN: i64 = 33;
/// ecdsa-with-SHA256 (1.2.840.10045.4.3.2).
const ECDSA_WITH_SHA256: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.10045.4.3.2");
/// id-at-commonName (2.5.4.3).
const AT_COMMON_NAME: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.4.3");

/// A `did:x509` trust policy over the leaf certificate. All non-empty constraints must hold.
#[derive(Debug, Clone, Default)]
pub struct DidX509Policy {
    /// Required leaf subject Common Name (exact match), if set.
    pub require_subject_cn: Option<String>,
    /// Required Extended Key Usage OIDs on the leaf (all must be present), as dotted strings.
    pub require_eku: Vec<String>,
    /// Required Subject Alternative Name DNS entries on the leaf (all must be present).
    pub require_san_dns: Vec<String>,
}

/// A trust anchor authorizing a `did:x509` issuer: a trusted CA (by SHA-256 fingerprint of
/// its DER) plus the policy the leaf must satisfy. `did` is the issuer id fragments must
/// declare and equals the canonical `did:x509` derived from this anchor.
#[derive(Debug, Clone)]
pub struct DidX509Anchor {
    pub did: String,
    pub ca_fingerprint: [u8; 32],
    pub policy: DidX509Policy,
}

fn fingerprint(der_bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(der_bytes);
    h.finalize().into()
}

/// SHA-256 fingerprint of a DER-encoded certificate (the value a `did:x509` anchor and the
/// revocation list are keyed on).
pub fn sha256_fingerprint(der_bytes: &[u8]) -> [u8; 32] {
    fingerprint(der_bytes)
}

/// Compute the SHA-256 fingerprint of the first `CERTIFICATE` block in a PEM string, for
/// configuring a CA anchor from a PEM cert instead of a raw fingerprint. Pure-Rust base64
/// decode (no network, no extra crate beyond `base64ct`).
pub fn ca_fingerprint_from_pem(pem: &str) -> Result<[u8; 32], FragmentError> {
    use base64ct::{Base64, Encoding};
    let mut b64 = String::new();
    let mut in_block = false;
    for line in pem.lines() {
        let t = line.trim();
        if t.starts_with("-----BEGIN CERTIFICATE-----") {
            in_block = true;
            continue;
        }
        if t.starts_with("-----END CERTIFICATE-----") {
            break;
        }
        if in_block {
            b64.push_str(t);
        }
    }
    if b64.is_empty() {
        return Err(FragmentError::InvalidCertChain);
    }
    let der = Base64::decode_vec(&b64).map_err(|_| FragmentError::InvalidCertChain)?;
    Ok(fingerprint(&der))
}

/// Extract the ordered DER certificates (leaf first) from a COSE_Sign1 `x5chain` header
/// (checked in both the protected and unprotected buckets). A single certificate may be a
/// bare byte string; a chain is an array of byte strings.
fn extract_x5chain(sign1: &coset::CoseSign1) -> Result<Vec<Vec<u8>>, FragmentError> {
    use coset::cbor::value::Value;
    let find = |rest: &[(coset::Label, Value)]| -> Option<Value> {
        rest.iter().find_map(|(l, v)| match l {
            coset::Label::Int(i) if *i == COSE_HEADER_X5CHAIN => Some(v.clone()),
            _ => None,
        })
    };
    let val = find(&sign1.protected.header.rest)
        .or_else(|| find(&sign1.unprotected.rest))
        .ok_or(FragmentError::InvalidCertChain)?;
    let certs = match val {
        Value::Bytes(b) => vec![b],
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                match v {
                    Value::Bytes(b) => out.push(b),
                    _ => return Err(FragmentError::InvalidCertChain),
                }
            }
            out
        }
        _ => return Err(FragmentError::InvalidCertChain),
    };
    if certs.is_empty() {
        return Err(FragmentError::InvalidCertChain);
    }
    Ok(certs)
}

/// Whether a COSE_Sign1 envelope carries an `x5chain` header (used to route to the
/// `did:x509` verification path without attempting a full verification first).
pub fn cose_has_x5chain(cose_sign1: &[u8]) -> bool {
    use coset::CborSerializable;
    match coset::CoseSign1::from_slice(cose_sign1) {
        Ok(sign1) => extract_x5chain(&sign1).is_ok(),
        Err(_) => false,
    }
}

/// P-256 verifying key from a certificate's SubjectPublicKeyInfo. Only EC P-256 is supported
/// (the code-signing algorithm we target); anything else is rejected fail-closed.
fn p256_key(cert: &Certificate) -> Result<P256VerifyingKey, FragmentError> {
    let spki = &cert.tbs_certificate.subject_public_key_info;
    if spki.algorithm.oid != ID_EC_PUBLIC_KEY {
        return Err(FragmentError::InvalidCertChain);
    }
    let der = spki.to_der().map_err(|_| FragmentError::InvalidCertChain)?;
    P256VerifyingKey::from_public_key_der(&der).map_err(|_| FragmentError::InvalidCertChain)
}

/// Whether a certificate asserts `basicConstraints: cA=TRUE` — required of every issuer
/// (intermediate/CA) in the chain so that a non-CA leaf cannot mint sub-certificates.
fn is_ca(cert: &Certificate) -> bool {
    if let Some(exts) = &cert.tbs_certificate.extensions {
        for ext in exts.iter() {
            if ext.extn_id == ID_CE_BASIC_CONSTRAINTS {
                if let Ok(bc) =
                    x509_cert::ext::pkix::BasicConstraints::from_der(ext.extn_value.as_bytes())
                {
                    return bc.ca;
                }
                return false;
            }
        }
    }
    false
}

/// Verify that `subject` was signed by `issuer` (ECDSA P-256 / SHA-256 over the subject's
/// TBSCertificate).
fn verify_link(subject: &Certificate, issuer: &Certificate) -> Result<(), FragmentError> {
    // The issuer must be a CA (basicConstraints cA=TRUE), else a plain leaf could act as an
    // intermediate and mint sub-certificates (privilege escalation).
    if !is_ca(issuer) {
        return Err(FragmentError::InvalidCertChain);
    }
    if subject.signature_algorithm.oid != ECDSA_WITH_SHA256 {
        return Err(FragmentError::InvalidCertChain);
    }
    let issuer_key = p256_key(issuer)?;
    let tbs = subject
        .tbs_certificate
        .to_der()
        .map_err(|_| FragmentError::InvalidCertChain)?;
    let sig_der = subject
        .signature
        .as_bytes()
        .ok_or(FragmentError::InvalidCertChain)?;
    let sig = P256Signature::from_der(sig_der).map_err(|_| FragmentError::InvalidCertChain)?;
    issuer_key
        .verify(&tbs, &sig)
        .map_err(|_| FragmentError::InvalidCertChain)
}

/// Reject a certificate whose validity window does not include the current time.
fn check_validity(cert: &Certificate) -> Result<(), FragmentError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| FragmentError::CertExpired)?;
    let nb = cert.tbs_certificate.validity.not_before.to_unix_duration();
    let na = cert.tbs_certificate.validity.not_after.to_unix_duration();
    if now < nb || now > na {
        return Err(FragmentError::CertExpired);
    }
    Ok(())
}

/// The leaf's subject Common Name, if present.
fn subject_cn(leaf: &Certificate) -> Option<String> {
    for rdn in leaf.tbs_certificate.subject.0.iter() {
        for atv in rdn.0.iter() {
            if atv.oid == AT_COMMON_NAME {
                if let Ok(s) = atv.value.decode_as::<der::asn1::Utf8StringRef>() {
                    return Some(s.as_str().to_string());
                }
                if let Ok(s) = atv.value.decode_as::<der::asn1::PrintableStringRef>() {
                    return Some(s.as_str().to_string());
                }
            }
        }
    }
    None
}

/// The dotted EKU OIDs asserted by the leaf.
fn leaf_ekus(leaf: &Certificate) -> HashSet<String> {
    let mut out = HashSet::new();
    if let Some(exts) = &leaf.tbs_certificate.extensions {
        for ext in exts.iter() {
            if ext.extn_id == ID_CE_EXT_KEY_USAGE {
                if let Ok(eku) =
                    x509_cert::ext::pkix::ExtendedKeyUsage::from_der(ext.extn_value.as_bytes())
                {
                    for oid in eku.0.iter() {
                        out.insert(oid.to_string());
                    }
                }
            }
        }
    }
    out
}

/// The leaf's DNS SubjectAltName entries.
fn leaf_san_dns(leaf: &Certificate) -> HashSet<String> {
    let mut out = HashSet::new();
    if let Some(exts) = &leaf.tbs_certificate.extensions {
        for ext in exts.iter() {
            if ext.extn_id == ID_CE_SUBJECT_ALT_NAME {
                if let Ok(san) =
                    x509_cert::ext::pkix::SubjectAltName::from_der(ext.extn_value.as_bytes())
                {
                    for gn in san.0.iter() {
                        if let x509_cert::ext::pkix::name::GeneralName::DnsName(dns) = gn {
                            out.insert(dns.as_str().to_string());
                        }
                    }
                }
            }
        }
    }
    out
}

fn policy_matches(policy: &DidX509Policy, leaf: &Certificate) -> bool {
    if let Some(cn) = &policy.require_subject_cn {
        if subject_cn(leaf).as_deref() != Some(cn.as_str()) {
            return false;
        }
    }
    if !policy.require_eku.is_empty() {
        let have = leaf_ekus(leaf);
        if !policy.require_eku.iter().all(|e| have.contains(e)) {
            return false;
        }
    }
    if !policy.require_san_dns.is_empty() {
        let have = leaf_san_dns(leaf);
        if !policy.require_san_dns.iter().all(|s| have.contains(s)) {
            return false;
        }
    }
    true
}

/// Verify a COSE_Sign1 fragment envelope that carries an `x5chain`, against the configured
/// `did:x509` anchors and revocation list. On success returns the matched anchor's `did`
/// (which the caller requires to equal `fragment.issuer`).
///
/// Steps (all fail-closed): parse the chain → for each anchor, locate its trusted CA in the
/// chain by fingerprint → path-validate leaf→…→CA (signatures + validity) → check none of the
/// chain certs is revoked → check the `did:x509` policy over the leaf → verify the COSE_Sign1
/// signature with the leaf key over the statement.
pub fn verify_x509_cose(
    anchors: &HashMap<String, DidX509Anchor>,
    revoked: &HashSet<[u8; 32]>,
    cose_sign1: &[u8],
    statement: &[u8],
) -> Result<String, FragmentError> {
    use coset::CborSerializable;

    if anchors.is_empty() {
        return Err(FragmentError::UntrustedCa);
    }
    let sign1 =
        coset::CoseSign1::from_slice(cose_sign1).map_err(|_| FragmentError::InvalidCertChain)?;

    // The signed payload must be exactly the fragment statement.
    match &sign1.payload {
        Some(p) if p.as_slice() == statement => {}
        _ => return Err(FragmentError::InvalidSignature),
    }

    let chain_der = extract_x5chain(&sign1)?;
    let mut fps = Vec::with_capacity(chain_der.len());
    let mut certs = Vec::with_capacity(chain_der.len());
    for der in &chain_der {
        fps.push(fingerprint(der));
        certs.push(Certificate::from_der(der).map_err(|_| FragmentError::InvalidCertChain)?);
    }

    // Revocation is independent of which anchor matches: any revoked cert in the chain fails.
    if fps.iter().any(|fp| revoked.contains(fp)) {
        return Err(FragmentError::RevokedCertificate);
    }

    // Find an anchor whose trusted CA fingerprint appears in the chain.
    for anchor in anchors.values() {
        let Some(ca_idx) = fps.iter().position(|fp| *fp == anchor.ca_fingerprint) else {
            continue;
        };

        // Path-validate leaf(0) → … → CA(ca_idx): each cert signed by the next, all in date.
        for i in 0..=ca_idx {
            check_validity(&certs[i])?;
            if i < ca_idx {
                verify_link(&certs[i], &certs[i + 1])?;
            }
        }

        // did:x509 policy over the leaf.
        if !policy_matches(&anchor.policy, &certs[0]) {
            return Err(FragmentError::DidX509Mismatch);
        }

        // Verify the COSE_Sign1 signature with the leaf key (ECDSA P-256 / SHA-256).
        let leaf_key = p256_key(&certs[0])?;
        sign1
            .verify_signature(b"", |sig, tbs| {
                let s = P256Signature::from_slice(sig).map_err(|_| ())?;
                leaf_key.verify(tbs, &s).map_err(|_| ())
            })
            .map_err(|_| FragmentError::InvalidSignature)?;

        return Ok(anchor.did.clone());
    }

    // No configured anchor's CA appeared in the presented chain.
    Err(FragmentError::UntrustedCa)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PolicyFragment;
    use coset::cbor::value::Value;
    use coset::{iana, CborSerializable, CoseSign1Builder, HeaderBuilder};
    use p256::ecdsa::signature::Signer;
    use p256::ecdsa::{DerSignature, Signature as EcSignature, SigningKey};
    use p256::pkcs8::EncodePublicKey;
    use rand_core::OsRng;
    use std::str::FromStr;
    use std::convert::TryFrom;
    use std::time::Duration;
    use x509_cert::builder::{Builder, CertificateBuilder, Profile};
    use x509_cert::ext::pkix::ExtendedKeyUsage;
    use x509_cert::name::Name;
    use x509_cert::serial_number::SerialNumber;
    use x509_cert::spki::SubjectPublicKeyInfoOwned;
    use x509_cert::time::{Time, Validity};

    const EKU_CODE_SIGNING: &str = "1.3.6.1.5.5.7.3.3";

    fn spki_of(sk: &SigningKey) -> SubjectPublicKeyInfoOwned {
        let der = sk.verifying_key().to_public_key_der().unwrap();
        SubjectPublicKeyInfoOwned::from_der(der.as_bytes()).unwrap()
    }

    fn mint_ca(cn: &str, sk: &SigningKey) -> Vec<u8> {
        let subject = Name::from_str(&format!("CN={cn}")).unwrap();
        let validity = Validity::from_now(Duration::from_secs(3600)).unwrap();
        let builder = CertificateBuilder::new(
            Profile::Root,
            SerialNumber::from(1u32),
            validity,
            subject,
            spki_of(sk),
            sk,
        )
        .unwrap();
        builder.build::<DerSignature>().unwrap().to_der().unwrap()
    }

    /// Mint a leaf signed by the CA key, with a code-signing EKU. `validity` lets tests mint
    /// an expired leaf.
    fn mint_leaf(cn: &str, leaf_sk: &SigningKey, ca_cn: &str, ca_sk: &SigningKey, validity: Validity) -> Vec<u8> {
        let issuer = Name::from_str(&format!("CN={ca_cn}")).unwrap();
        let subject = Name::from_str(&format!("CN={cn}")).unwrap();
        let mut builder = CertificateBuilder::new(
            Profile::Leaf {
                issuer,
                enable_key_agreement: false,
                enable_key_encipherment: false,
            },
            SerialNumber::from(2u32),
            validity,
            subject,
            spki_of(leaf_sk),
            ca_sk,
        )
        .unwrap();
        let eku = ExtendedKeyUsage(vec![ObjectIdentifier::new_unwrap(EKU_CODE_SIGNING)]);
        builder.add_extension(&eku).unwrap();
        builder.build::<DerSignature>().unwrap().to_der().unwrap()
    }

    /// Mint an intermediate CA (basicConstraints cA=TRUE) signed by the root CA key.
    fn mint_intermediate(cn: &str, int_sk: &SigningKey, root_cn: &str, root_sk: &SigningKey) -> Vec<u8> {
        let issuer = Name::from_str(&format!("CN={root_cn}")).unwrap();
        let subject = Name::from_str(&format!("CN={cn}")).unwrap();
        let validity = Validity::from_now(Duration::from_secs(3600)).unwrap();
        let builder = CertificateBuilder::new(
            Profile::SubCA { issuer, path_len_constraint: None },
            SerialNumber::from(3u32),
            validity,
            subject,
            spki_of(int_sk),
            root_sk,
        )
        .unwrap();
        builder.build::<DerSignature>().unwrap().to_der().unwrap()
    }

    fn cose_with_chain(statement: &[u8], leaf_sk: &SigningKey, chain: &[Vec<u8>]) -> Vec<u8> {
        let protected = HeaderBuilder::new()
            .algorithm(iana::Algorithm::ES256)
            .build();
        let mut unprotected = coset::Header::default();
        unprotected.rest.push((
            coset::Label::Int(COSE_HEADER_X5CHAIN),
            Value::Array(chain.iter().map(|c| Value::Bytes(c.clone())).collect()),
        ));
        CoseSign1Builder::new()
            .protected(protected)
            .unprotected(unprotected)
            .payload(statement.to_vec())
            .create_signature(b"", |tbs| {
                let sig: EcSignature = leaf_sk.sign(tbs);
                sig.to_bytes().to_vec()
            })
            .build()
            .to_vec()
            .unwrap()
    }

    fn anchor_for(ca_der: &[u8], did: &str) -> DidX509Anchor {
        DidX509Anchor {
            did: did.to_string(),
            ca_fingerprint: fingerprint(ca_der),
            policy: DidX509Policy {
                require_eku: vec![EKU_CODE_SIGNING.to_string()],
                ..Default::default()
            },
        }
    }

    fn frag(issuer: &str) -> PolicyFragment {
        PolicyFragment {
            issuer: issuer.to_string(),
            svn: 1,
            ..Default::default()
        }
    }

    /// TC-F1.9: a leaf under a trusted CA, satisfying the did:x509 policy, verifies; the
    /// derived did equals the anchor did.
    #[test]
    fn tc_f1_9_valid_chain_accepted() {
        let ca_sk = SigningKey::random(&mut OsRng);
        let leaf_sk = SigningKey::random(&mut OsRng);
        let ca = mint_ca("test-ca", &ca_sk);
        let leaf = mint_leaf("issuerX", &leaf_sk, "test-ca", &ca_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        let anchor = anchor_for(&ca, "did:x509:test:issuerX");

        let f = frag("did:x509:test:issuerX");
        let cose = cose_with_chain(&f.signing_bytes(), &leaf_sk, &[leaf, ca]);
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);
        let did = verify_x509_cose(&anchors, &HashSet::new(), &cose, &f.signing_bytes()).unwrap();
        assert_eq!(did, "did:x509:test:issuerX");
    }

    /// TC-F1.10: an untrusted CA (fingerprint not configured) is rejected.
    #[test]
    fn tc_f1_10_untrusted_ca_rejected() {
        let ca_sk = SigningKey::random(&mut OsRng);
        let other_sk = SigningKey::random(&mut OsRng);
        let leaf_sk = SigningKey::random(&mut OsRng);
        let ca = mint_ca("test-ca", &ca_sk);
        let other_ca = mint_ca("other-ca", &other_sk);
        let leaf = mint_leaf("issuerX", &leaf_sk, "test-ca", &ca_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        // Anchor trusts a different CA than the one in the chain.
        let anchor = anchor_for(&other_ca, "did:x509:test:issuerX");

        let f = frag("did:x509:test:issuerX");
        let cose = cose_with_chain(&f.signing_bytes(), &leaf_sk, &[leaf, ca]);
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);
        assert_eq!(
            verify_x509_cose(&anchors, &HashSet::new(), &cose, &f.signing_bytes()).unwrap_err(),
            FragmentError::UntrustedCa
        );
    }

    /// TC-F1.10b: a broken leaf signature (wrong signing key over the COSE) is rejected.
    #[test]
    fn tc_f1_10b_broken_signature_rejected() {
        let ca_sk = SigningKey::random(&mut OsRng);
        let leaf_sk = SigningKey::random(&mut OsRng);
        let attacker_sk = SigningKey::random(&mut OsRng);
        let ca = mint_ca("test-ca", &ca_sk);
        let leaf = mint_leaf("issuerX", &leaf_sk, "test-ca", &ca_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        let anchor = anchor_for(&ca, "did:x509:test:issuerX");

        let f = frag("did:x509:test:issuerX");
        // COSE signed by an attacker key, not the leaf's key.
        let cose = cose_with_chain(&f.signing_bytes(), &attacker_sk, &[leaf, ca]);
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);
        assert_eq!(
            verify_x509_cose(&anchors, &HashSet::new(), &cose, &f.signing_bytes()).unwrap_err(),
            FragmentError::InvalidSignature
        );
    }

    /// TC-F1.10c: an expired leaf is rejected.
    #[test]
    fn tc_f1_10c_expired_leaf_rejected() {
        let ca_sk = SigningKey::random(&mut OsRng);
        let leaf_sk = SigningKey::random(&mut OsRng);
        let ca = mint_ca("test-ca", &ca_sk);
        // Validity window entirely in the past.
        let past = Validity {
            not_before: Time::try_from(std::time::UNIX_EPOCH + Duration::from_secs(1_000_000_000)).unwrap(),
            not_after: Time::try_from(std::time::UNIX_EPOCH + Duration::from_secs(1_000_100_000)).unwrap(),
        };
        let leaf = mint_leaf("issuerX", &leaf_sk, "test-ca", &ca_sk, past);
        let anchor = anchor_for(&ca, "did:x509:test:issuerX");

        let f = frag("did:x509:test:issuerX");
        let cose = cose_with_chain(&f.signing_bytes(), &leaf_sk, &[leaf, ca]);
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);
        assert_eq!(
            verify_x509_cose(&anchors, &HashSet::new(), &cose, &f.signing_bytes()).unwrap_err(),
            FragmentError::CertExpired
        );
    }

    /// TC-F1.11: a revoked leaf (fingerprint on the measured list) is rejected even with a
    /// valid chain and signature.
    #[test]
    fn tc_f1_11_revoked_leaf_rejected() {
        let ca_sk = SigningKey::random(&mut OsRng);
        let leaf_sk = SigningKey::random(&mut OsRng);
        let ca = mint_ca("test-ca", &ca_sk);
        let leaf = mint_leaf("issuerX", &leaf_sk, "test-ca", &ca_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        let anchor = anchor_for(&ca, "did:x509:test:issuerX");

        let f = frag("did:x509:test:issuerX");
        let cose = cose_with_chain(&f.signing_bytes(), &leaf_sk, &[leaf.clone(), ca]);
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);
        let mut revoked = HashSet::new();
        revoked.insert(fingerprint(&leaf));
        assert_eq!(
            verify_x509_cose(&anchors, &revoked, &cose, &f.signing_bytes()).unwrap_err(),
            FragmentError::RevokedCertificate
        );
    }

    /// TC-F1.12: a rotated leaf (new key + cert, same CA and policy) verifies with no
    /// anchor/config change — trust is anchored on the CA + policy, not the leaf key.
    #[test]
    fn tc_f1_12_rotated_leaf_accepted() {
        let ca_sk = SigningKey::random(&mut OsRng);
        let ca = mint_ca("test-ca", &ca_sk);
        let anchor = anchor_for(&ca, "did:x509:test:issuerX");
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);

        // First leaf.
        let leaf1_sk = SigningKey::random(&mut OsRng);
        let leaf1 = mint_leaf("issuerX", &leaf1_sk, "test-ca", &ca_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        let f1 = frag("did:x509:test:issuerX");
        let cose1 = cose_with_chain(&f1.signing_bytes(), &leaf1_sk, &[leaf1, ca.clone()]);
        assert!(verify_x509_cose(&anchors, &HashSet::new(), &cose1, &f1.signing_bytes()).is_ok());

        // Rotated leaf: brand-new key, same CA + policy, no config change.
        let leaf2_sk = SigningKey::random(&mut OsRng);
        let leaf2 = mint_leaf("issuerX", &leaf2_sk, "test-ca", &ca_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        let f2 = frag("did:x509:test:issuerX");
        let cose2 = cose_with_chain(&f2.signing_bytes(), &leaf2_sk, &[leaf2, ca]);
        assert!(verify_x509_cose(&anchors, &HashSet::new(), &cose2, &f2.signing_bytes()).is_ok());
    }

    /// TC-F1.12d: the did:x509 policy is enforced — a leaf missing the required EKU is
    /// rejected as a did:x509 mismatch even though the chain is otherwise valid.
    #[test]
    fn tc_f1_12d_policy_mismatch_rejected() {
        let ca_sk = SigningKey::random(&mut OsRng);
        let leaf_sk = SigningKey::random(&mut OsRng);
        let ca = mint_ca("test-ca", &ca_sk);
        let leaf = mint_leaf("issuerX", &leaf_sk, "test-ca", &ca_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        // Anchor requires an EKU the leaf does not carry (server-auth instead of code-signing).
        let anchor = DidX509Anchor {
            did: "did:x509:test:issuerX".to_string(),
            ca_fingerprint: fingerprint(&ca),
            policy: DidX509Policy {
                require_eku: vec!["1.3.6.1.5.5.7.3.1".to_string()],
                ..Default::default()
            },
        };
        let f = frag("did:x509:test:issuerX");
        let cose = cose_with_chain(&f.signing_bytes(), &leaf_sk, &[leaf, ca]);
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);
        assert_eq!(
            verify_x509_cose(&anchors, &HashSet::new(), &cose, &f.signing_bytes()).unwrap_err(),
            FragmentError::DidX509Mismatch
        );
    }

    /// TC-F1.13: a 3-cert chain (leaf ← intermediate CA ← root CA), anchored on the root
    /// fingerprint, path-validates through the intermediate and is accepted.
    #[test]
    fn tc_f1_13_intermediate_ca_chain_accepted() {
        let root_sk = SigningKey::random(&mut OsRng);
        let int_sk = SigningKey::random(&mut OsRng);
        let leaf_sk = SigningKey::random(&mut OsRng);
        let root = mint_ca("root-ca", &root_sk);
        let intermediate = mint_intermediate("int-ca", &int_sk, "root-ca", &root_sk);
        let leaf = mint_leaf("issuerX", &leaf_sk, "int-ca", &int_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        // Trust anchored on the ROOT fingerprint; the intermediate is validated in between.
        let anchor = anchor_for(&root, "did:x509:test:issuerX");
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);

        let f = frag("did:x509:test:issuerX");
        let cose = cose_with_chain(&f.signing_bytes(), &leaf_sk, &[leaf, intermediate, root]);
        let did = verify_x509_cose(&anchors, &HashSet::new(), &cose, &f.signing_bytes()).unwrap();
        assert_eq!(did, "did:x509:test:issuerX");
    }

    /// TC-F1.13b: a chain whose "intermediate" is a non-CA leaf (basicConstraints cA=FALSE)
    /// is rejected — a plain leaf cannot act as an issuer and mint sub-certificates.
    #[test]
    fn tc_f1_13b_non_ca_intermediate_rejected() {
        let root_sk = SigningKey::random(&mut OsRng);
        let mid_sk = SigningKey::random(&mut OsRng); // a LEAF (cA=FALSE), misused as an issuer
        let subleaf_sk = SigningKey::random(&mut OsRng);
        let root = mint_ca("root-ca", &root_sk);
        let mid_leaf = mint_leaf("mid", &mid_sk, "root-ca", &root_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        let subleaf = mint_leaf("issuerX", &subleaf_sk, "mid", &mid_sk, Validity::from_now(Duration::from_secs(3600)).unwrap());
        let anchor = anchor_for(&root, "did:x509:test:issuerX");
        let mut anchors = HashMap::new();
        anchors.insert(anchor.did.clone(), anchor);

        let f = frag("did:x509:test:issuerX");
        let cose = cose_with_chain(&f.signing_bytes(), &subleaf_sk, &[subleaf, mid_leaf, root]);
        assert_eq!(
            verify_x509_cose(&anchors, &HashSet::new(), &cose, &f.signing_bytes()).unwrap_err(),
            FragmentError::InvalidCertChain
        );
    }
}
