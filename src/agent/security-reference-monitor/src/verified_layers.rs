// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-4C — verified read-only layers (dm-verity root-hash authorization).
//!
//! Kata's agent already builds dm-verity targets for read-only rootfs (EROFS) layers, so the
//! kernel cryptographically enforces that a layer's contents hash to a given root digest.
//! What was missing is *authorization of that digest*: nothing checked that the root hash the
//! (untrusted) host supplied is one the tenant/policy approved. Without it, a malicious host
//! can serve its own layer together with the matching, self-computed root hash — dm-verity
//! passes and the attacker-controlled layer is mounted read-only.
//!
//! This module is the Kata equivalent of runhcs/OpenGCS `EnforceDeviceMountPolicy(target,
//! RootDigest)`: it holds a **measured allowlist** of approved `(algorithm, root_hash)` pairs
//! and a fail-closed gate that the storage handler calls *before* creating the dm-verity
//! device. Combined with the kernel's dm-verity content check, this gives the full guarantee:
//! the layer's bytes match a digest the tenant approved.
//!
//! Fail-closed semantics (mirrors the FR-1 fragment trust root):
//!   - `require == false` (feature off / opt-in): every layer is allowed (no enforcement).
//!   - `require == true` and the allowlist is **empty**: every layer is rejected
//!     (`NoApprovedLayers`) — an absent/empty measured config must not open the gate.
//!   - `require == true` and non-empty: only `(algorithm, root_hash)` pairs in the allowlist
//!     are accepted; anything else is `UnauthorizedLayer`.
//!
//! Comparisons are over normalized values (trimmed, lower-cased) so hex-case / whitespace
//! differences between the measured config and the host-supplied option cannot cause a
//! spurious accept or reject.

use std::collections::HashSet;
use std::fmt;

/// Verifier for read-only layer (dm-verity) root digests against a measured allowlist.
#[derive(Debug, Default)]
pub struct VerifiedLayerStore {
    /// Approved `(algorithm, root_hash)` pairs, normalized (trimmed, lower-cased).
    approved: HashSet<(String, String)>,
    /// When true, a presented layer must be in the allowlist (fail-closed on empty).
    require: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LayerError {
    /// The presented `(algorithm, root_hash)` is not in the measured allowlist.
    UnauthorizedLayer { algorithm: String, root_hash: String },
    /// Verification is required but no layer has been authorized (fail-closed): an
    /// absent/empty measured config must reject every layer, not open the gate.
    NoApprovedLayers,
}

impl fmt::Display for LayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayerError::UnauthorizedLayer { algorithm, root_hash } => write!(
                f,
                "unauthorized dm-verity layer: algorithm {algorithm}, root_hash {root_hash}"
            ),
            LayerError::NoApprovedLayers => {
                write!(f, "dm-verity layer verification required but no layer is authorized")
            }
        }
    }
}

impl std::error::Error for LayerError {}

fn normalize(algorithm: &str, root_hash: &str) -> (String, String) {
    (
        algorithm.trim().to_ascii_lowercase(),
        root_hash.trim().to_ascii_lowercase(),
    )
}

impl VerifiedLayerStore {
    /// Create a store. `require` should be true in strict mode when a verified-layer policy is
    /// in effect; it is set from measured config.
    pub fn new(require: bool) -> Self {
        Self {
            require,
            ..Default::default()
        }
    }

    /// Set whether verification is required (fail-closed when the allowlist is empty).
    pub fn set_require(&mut self, require: bool) {
        self.require = require;
    }

    /// Whether verification is currently required.
    pub fn is_required(&self) -> bool {
        self.require
    }

    /// Authorize a read-only layer by its dm-verity `(algorithm, root_hash)` from measured
    /// state. A layer whose effective root digest matches an authorized pair is accepted.
    pub fn authorize_layer(&mut self, algorithm: &str, root_hash: &str) {
        self.approved.insert(normalize(algorithm, root_hash));
    }

    /// Whether any layer is authorized (fail-closed indicator).
    pub fn has_authorized_layers(&self) -> bool {
        !self.approved.is_empty()
    }

    /// Number of authorized layers.
    pub fn len(&self) -> usize {
        self.approved.len()
    }

    /// Whether the allowlist is empty.
    pub fn is_empty(&self) -> bool {
        self.approved.is_empty()
    }

    /// Authorize a read-only layer, verifying (fail-closed) that its `(algorithm, root_hash)`
    /// is in the measured allowlist. Called by the storage handler *before* the dm-verity
    /// device is created. When verification is not required, always succeeds (opt-in).
    pub fn verify(&self, algorithm: &str, root_hash: &str) -> Result<(), LayerError> {
        if !self.require {
            return Ok(());
        }
        if self.approved.is_empty() {
            return Err(LayerError::NoApprovedLayers);
        }
        let key = normalize(algorithm, root_hash);
        if self.approved.contains(&key) {
            Ok(())
        } else {
            Err(LayerError::UnauthorizedLayer {
                algorithm: key.0,
                root_hash: key.1,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT_A: &str = "aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44";
    const ROOT_B: &str = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";

    /// TC-F4C.1: an authorized (algorithm, root_hash) is accepted; a different hash under the
    /// same algorithm is rejected as unauthorized.
    #[test]
    fn authorized_layer_accepted_unauthorized_rejected() {
        let mut store = VerifiedLayerStore::new(true);
        store.authorize_layer("sha256", ROOT_A);
        assert!(store.verify("sha256", ROOT_A).is_ok());
        assert_eq!(
            store.verify("sha256", ROOT_B).unwrap_err(),
            LayerError::UnauthorizedLayer {
                algorithm: "sha256".into(),
                root_hash: ROOT_B.into(),
            }
        );
    }

    /// TC-F4C.2: verification required with an empty allowlist rejects every layer
    /// (fail-closed): an absent/empty measured config must not open the gate.
    #[test]
    fn empty_allowlist_is_fail_closed_when_required() {
        let store = VerifiedLayerStore::new(true);
        assert!(!store.has_authorized_layers());
        assert_eq!(store.verify("sha256", ROOT_A).unwrap_err(), LayerError::NoApprovedLayers);
    }

    /// TC-F4C.3: when verification is not required (feature off / opt-in), any layer is
    /// allowed — preserving existing behavior in non-strict/disabled configurations.
    #[test]
    fn not_required_allows_any_layer() {
        let store = VerifiedLayerStore::new(false);
        assert!(store.verify("sha256", ROOT_A).is_ok());
        assert!(store.verify("sha512", "deadbeef").is_ok());
    }

    /// TC-F4C.4: the algorithm is part of the key — the right hash under the wrong algorithm
    /// is rejected (a host cannot present an authorized hash tagged with a weaker algorithm).
    #[test]
    fn algorithm_is_bound() {
        let mut store = VerifiedLayerStore::new(true);
        store.authorize_layer("sha256", ROOT_A);
        assert!(matches!(
            store.verify("sha512", ROOT_A).unwrap_err(),
            LayerError::UnauthorizedLayer { .. }
        ));
    }

    /// TC-F4C.5: comparison is normalized (hex case + surrounding whitespace) so cosmetic
    /// differences between the measured config and the host option cannot spuriously accept
    /// or reject.
    #[test]
    fn comparison_is_normalized() {
        let mut store = VerifiedLayerStore::new(true);
        store.authorize_layer("SHA256", &ROOT_A.to_ascii_uppercase());
        // Host presents lower-case with padding and mixed-case algorithm.
        assert!(store.verify("  sha256  ", &format!("  {ROOT_A}  ")).is_ok());
    }

    /// TC-F4C.6: multiple approved layers (a multi-layer image) each verify.
    #[test]
    fn multiple_layers_authorized() {
        let mut store = VerifiedLayerStore::new(true);
        store.authorize_layer("sha256", ROOT_A);
        store.authorize_layer("sha256", ROOT_B);
        assert_eq!(store.len(), 2);
        assert!(store.verify("sha256", ROOT_A).is_ok());
        assert!(store.verify("sha256", ROOT_B).is_ok());
    }
}
