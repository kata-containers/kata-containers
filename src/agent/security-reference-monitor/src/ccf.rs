// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! BL-6 — SCITT / CCF-profile transparency-log inclusion proofs.
//!
//! FR-1f Stage 2 already verifies our native RFC 6962 `kata-ttl-proof/v1` (self-chained,
//! mock-ledger). This module adds interoperability with a **real external transparency
//! ledger** — the SCITT CCF profile used by Azure Confidential Ledger and CCF-based SCITT
//! services (draft-ietf-scitt-receipts-ccf-profile), the same profile the reference
//! confidential runtime consumes. It recomputes the Merkle root from a CCF `ccf-inclusion-proof`
//! and returns the leaf `data-hash`, so the caller can (a) require `data-hash == SHA-256(signed
//! statement)` and (b) verify the ledger's signature over the recomputed root.
//!
//! CCF construction (note: **not** RFC 6962 — no `0x00`/`0x01` domain-separation prefixes):
//! ```text
//! leaf_hash = SHA-256( internal_transaction_hash(32) || SHA-256(internal_evidence) || data_hash(32) )
//! for each path element [left: bool, sibling: 32]:
//!     h = left ? SHA-256(sibling || h) : SHA-256(h || sibling)
//! root = h
//! ```
//!
//! `ccf-inclusion-proof = { 1 => [tx_hash, evidence, data_hash], 2 => [ [left, hash], ... ] }`
//! (CBOR). Pure-Rust (`ciborium` + `sha2`); no network, no Go.

use ciborium::value::Value;
use sha2::{Digest, Sha256};
use std::convert::TryFrom;

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// CCF leaf hash: `SHA-256(internal_tx_hash || SHA-256(internal_evidence) || data_hash)`.
pub fn ccf_leaf_hash(internal_tx_hash: &[u8; 32], internal_evidence: &[u8], data_hash: &[u8; 32]) -> [u8; 32] {
    let ev = sha256(&[internal_evidence]);
    sha256(&[internal_tx_hash, &ev, data_hash])
}

fn as_bytes32(v: &Value) -> Option<[u8; 32]> {
    let b = v.as_bytes()?;
    if b.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(b);
    Some(out)
}

/// Recompute the Merkle root and extract the leaf `data-hash` from a CBOR-encoded
/// `ccf-inclusion-proof`. Returns `(root, data_hash)` or `None` on any malformed input.
pub fn ccf_root_and_data_hash(proof_cbor: &[u8]) -> Option<([u8; 32], [u8; 32])> {
    let proof: Value = ciborium::from_reader(proof_cbor).ok()?;
    let map = proof.as_map()?;
    // Keyed by integer: 1 => leaf, 2 => path.
    let mut leaf: Option<&Value> = None;
    let mut path: Option<&Value> = None;
    for (k, val) in map {
        match k.as_integer().and_then(|i| i64::try_from(i).ok()) {
            Some(1) => leaf = Some(val),
            Some(2) => path = Some(val),
            _ => {}
        }
    }
    // ccf-leaf = [ internal_tx_hash: bstr(32), internal_evidence: tstr(1..1024), data_hash: bstr(32) ]
    let leaf_arr = leaf?.as_array()?;
    if leaf_arr.len() != 3 {
        return None;
    }
    let internal_tx_hash = as_bytes32(&leaf_arr[0])?;
    let internal_evidence = leaf_arr[1].as_text()?;
    if internal_evidence.is_empty() || internal_evidence.len() > 1024 {
        return None;
    }
    let data_hash = as_bytes32(&leaf_arr[2])?;

    let mut h = ccf_leaf_hash(&internal_tx_hash, internal_evidence.as_bytes(), &data_hash);

    // ccf-proof-element = [ left: bool, hash: bstr(32) ]; path must be non-empty.
    let path_arr = path?.as_array()?;
    if path_arr.is_empty() {
        return None;
    }
    for el in path_arr {
        let el = el.as_array()?;
        if el.len() != 2 {
            return None;
        }
        let left = match &el[0] {
            Value::Bool(b) => *b,
            _ => return None,
        };
        let sib = as_bytes32(&el[1])?;
        h = if left {
            sha256(&[&sib, &h])
        } else {
            sha256(&[&h, &sib])
        };
    }
    Some((h, data_hash))
}

/// Verify a CCF inclusion proof binds `expected_data_hash` and return the Merkle root the
/// ledger's receipt must be signed over. The caller passes `SHA-256(signed statement)` as
/// `expected_data_hash` and then checks the ledger signature over the returned root.
pub fn verify_ccf_inclusion(proof_cbor: &[u8], expected_data_hash: &[u8; 32]) -> Option<[u8; 32]> {
    let (root, data_hash) = ccf_root_and_data_hash(proof_cbor)?;
    if &data_hash == expected_data_hash {
        Some(root)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build a CBOR ccf-inclusion-proof for `data_hash` with a single sibling on the right
    // (left=false): root = SHA-256(leaf || sib).
    fn make_proof(data_hash: &[u8; 32], sib: &[u8; 32], left: bool, evidence: &str) -> (Vec<u8>, [u8; 32]) {
        let tx = [7u8; 32];
        let leaf = ccf_leaf_hash(&tx, evidence.as_bytes(), data_hash);
        let root = if left { sha256(&[sib, &leaf]) } else { sha256(&[&leaf, sib]) };
        let proof = Value::Map(vec![
            (
                Value::Integer(1.into()),
                Value::Array(vec![
                    Value::Bytes(tx.to_vec()),
                    Value::Text(evidence.to_string()),
                    Value::Bytes(data_hash.to_vec()),
                ]),
            ),
            (
                Value::Integer(2.into()),
                Value::Array(vec![Value::Array(vec![
                    Value::Bool(left),
                    Value::Bytes(sib.to_vec()),
                ])]),
            ),
        ]);
        let mut buf = Vec::new();
        ciborium::into_writer(&proof, &mut buf).unwrap();
        (buf, root)
    }

    #[test]
    fn ccf_inclusion_recomputes_root_and_binds_data_hash() {
        let data_hash = [0xabu8; 32];
        let sib = [0x11u8; 32];
        let (proof, root) = make_proof(&data_hash, &sib, false, "ccf-evidence");
        // Correct data-hash → returns the same root the producer computed.
        assert_eq!(verify_ccf_inclusion(&proof, &data_hash), Some(root));
        // Wrong expected data-hash → rejected (the proof does not bind our statement).
        assert_eq!(verify_ccf_inclusion(&proof, &[0u8; 32]), None);
    }

    #[test]
    fn ccf_left_sibling_folds_correctly() {
        let data_hash = [0x5au8; 32];
        let sib = [0x22u8; 32];
        let (proof, root) = make_proof(&data_hash, &sib, true, "e");
        assert_eq!(verify_ccf_inclusion(&proof, &data_hash), Some(root));
    }

    #[test]
    fn ccf_malformed_proof_rejected() {
        // Not CBOR.
        assert!(ccf_root_and_data_hash(b"not-cbor").is_none());
        // Empty path is rejected.
        let bad = Value::Map(vec![
            (
                Value::Integer(1.into()),
                Value::Array(vec![
                    Value::Bytes(vec![0u8; 32]),
                    Value::Text("e".into()),
                    Value::Bytes(vec![0u8; 32]),
                ]),
            ),
            (Value::Integer(2.into()), Value::Array(vec![])),
        ]);
        let mut buf = Vec::new();
        ciborium::into_writer(&bad, &mut buf).unwrap();
        assert!(ccf_root_and_data_hash(&buf).is_none());
    }
}
