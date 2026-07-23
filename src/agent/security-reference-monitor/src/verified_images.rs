// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! BL-3 — verified guest-pull images (authorize the image manifest digest).
//!
//! In the confidential guest-pull path the agent asks the Confidential Data Hub (CDH) to pull
//! and unpack a container image referenced by the (untrusted) host. The CDH/registry verify
//! that the pulled content matches the referenced digest, but nothing in the guest checked
//! that the *reference* resolves to a digest the tenant approved — so a host could point the
//! workload at a different (self-consistent) image.
//!
//! This module is the image-path analogue of FR-4C (verified layers): it holds a **measured
//! allowlist** of authorized image manifest digests and a fail-closed gate the storage
//! handler calls *before* the pull. Combined with pull-by-digest (content ↔ digest verified
//! by the registry/CDH), this binds the workload to a tenant-approved image.
//!
//! Fail-closed semantics (mirrors FR-1 fragments / FR-4C):
//!   - `require == false` (feature off / opt-in): any image is allowed (no enforcement).
//!   - `require == true` and the reference is **not pinned by digest** (`name@alg:hex`):
//!     rejected (`UnpinnedImage`) — a tag alone is not a stable identity.
//!   - `require == true` and the allowlist is **empty**: every image is rejected
//!     (`NoApprovedImages`) — an absent/empty measured config must not open the gate.
//!   - `require == true` and the pinned digest is not in the allowlist: `UnauthorizedImage`.
//!
//! Digests are compared normalized (trimmed, lower-cased, `algorithm:hex`).

use std::collections::HashSet;
use std::fmt;

/// Verifier for guest-pull image references against a measured allowlist of manifest digests.
#[derive(Debug, Default)]
pub struct VerifiedImageStore {
    /// Approved image manifest digests, normalized `algorithm:hex` (lower-cased).
    approved: HashSet<String>,
    /// When true, a pulled image must be pinned by digest and in the allowlist (fail-closed).
    require: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ImageError {
    /// The image reference's manifest digest is not in the measured allowlist.
    UnauthorizedImage { digest: String },
    /// Verification is required but the reference is not pinned by digest (`name@alg:hex`).
    UnpinnedImage { image: String },
    /// Verification is required but no image is authorized (fail-closed).
    NoApprovedImages,
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::UnauthorizedImage { digest } => {
                write!(f, "unauthorized guest-pull image digest: {digest}")
            }
            ImageError::UnpinnedImage { image } => write!(
                f,
                "guest-pull image is not pinned by digest (name@alg:hex): {image}"
            ),
            ImageError::NoApprovedImages => {
                write!(f, "guest-pull image verification required but no image is authorized")
            }
        }
    }
}

impl std::error::Error for ImageError {}

/// Parse and normalize the digest (`algorithm:hex`) pinned in an OCI image reference of the
/// form `name@algorithm:hex`. Returns `None` if the reference is not pinned by a well-formed
/// `sha256`/`sha384`/`sha512` digest.
pub fn parse_pinned_digest(image_ref: &str) -> Option<String> {
    let (_, digest) = image_ref.trim().rsplit_once('@')?;
    let (alg, hex) = digest.split_once(':')?;
    let alg = alg.trim().to_ascii_lowercase();
    let hex = hex.trim().to_ascii_lowercase();
    let expected_len = match alg.as_str() {
        "sha256" => 64,
        "sha384" => 96,
        "sha512" => 128,
        _ => return None,
    };
    if hex.len() != expected_len || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("{alg}:{hex}"))
}

impl VerifiedImageStore {
    /// Create a store. `require` should be true in strict mode when an image allowlist is in
    /// effect; it is set from measured config.
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

    /// Authorize an image by its manifest digest (`algorithm:hex`) from measured state.
    /// Malformed digests are ignored (the entry simply never matches).
    pub fn authorize_image(&mut self, digest: &str) {
        let d = digest.trim().to_ascii_lowercase();
        if let Some((alg, hex)) = d.split_once(':') {
            if !alg.is_empty() && !hex.is_empty() {
                self.approved.insert(format!("{alg}:{hex}"));
            }
        }
    }

    /// Whether any image is authorized (fail-closed indicator).
    pub fn has_authorized_images(&self) -> bool {
        !self.approved.is_empty()
    }

    /// Number of authorized images.
    pub fn len(&self) -> usize {
        self.approved.len()
    }

    /// Whether the allowlist is empty.
    pub fn is_empty(&self) -> bool {
        self.approved.is_empty()
    }

    /// Authorize a guest-pull image reference, verifying (fail-closed) that it is pinned by an
    /// approved manifest digest. Called by the image-pull storage handler *before* the pull.
    /// When verification is not required, always succeeds (opt-in).
    pub fn verify(&self, image_ref: &str) -> Result<(), ImageError> {
        if !self.require {
            return Ok(());
        }
        let digest = parse_pinned_digest(image_ref).ok_or_else(|| ImageError::UnpinnedImage {
            image: image_ref.trim().to_string(),
        })?;
        if self.approved.is_empty() {
            return Err(ImageError::NoApprovedImages);
        }
        if self.approved.contains(&digest) {
            Ok(())
        } else {
            Err(ImageError::UnauthorizedImage { digest })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D1: &str = "sha256:aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44";
    const D2: &str = "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
    const IMG1: &str = "registry.example/app@sha256:aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44";

    /// TC-BL3.1: an image pinned by an authorized digest is accepted; a different pinned
    /// digest is rejected.
    #[test]
    fn authorized_image_accepted_unauthorized_rejected() {
        let mut store = VerifiedImageStore::new(true);
        store.authorize_image(D1);
        assert!(store.verify(IMG1).is_ok());
        let other = format!("registry.example/app@{D2}");
        assert_eq!(
            store.verify(&other).unwrap_err(),
            ImageError::UnauthorizedImage { digest: D2.to_string() }
        );
    }

    /// TC-BL3.2: when required, a tag-only (unpinned) reference is rejected.
    #[test]
    fn unpinned_reference_rejected_when_required() {
        let mut store = VerifiedImageStore::new(true);
        store.authorize_image(D1);
        assert_eq!(
            store.verify("registry.example/app:latest").unwrap_err(),
            ImageError::UnpinnedImage { image: "registry.example/app:latest".into() }
        );
    }

    /// TC-BL3.3: required with an empty allowlist rejects every (even pinned) image
    /// (fail-closed).
    #[test]
    fn empty_allowlist_is_fail_closed_when_required() {
        let store = VerifiedImageStore::new(true);
        assert!(!store.has_authorized_images());
        assert_eq!(store.verify(IMG1).unwrap_err(), ImageError::NoApprovedImages);
    }

    /// TC-BL3.4: when not required (feature off), any reference (even tag-only) is allowed.
    #[test]
    fn not_required_allows_any_image() {
        let store = VerifiedImageStore::new(false);
        assert!(store.verify("registry.example/app:latest").is_ok());
        assert!(store.verify(IMG1).is_ok());
    }

    /// TC-BL3.5: digest comparison is normalized (hex case) and the algorithm is part of the
    /// key (a sha512 digest with the same hex prefix does not match a sha256 entry).
    #[test]
    fn comparison_is_normalized_and_algorithm_bound() {
        let mut store = VerifiedImageStore::new(true);
        store.authorize_image(&D1.to_ascii_uppercase());
        let upper = format!("registry.example/app@{}", D1.to_ascii_uppercase());
        assert!(store.verify(&upper).is_ok());
        // Malformed / wrong-length digest is treated as unpinned.
        assert!(matches!(
            store.verify("registry.example/app@sha256:deadbeef").unwrap_err(),
            ImageError::UnpinnedImage { .. }
        ));
    }

    /// TC-BL3.6: multiple approved images each verify.
    #[test]
    fn multiple_images_authorized() {
        let mut store = VerifiedImageStore::new(true);
        store.authorize_image(D1);
        store.authorize_image(D2);
        assert_eq!(store.len(), 2);
        assert!(store.verify(&format!("r/a@{D1}")).is_ok());
        assert!(store.verify(&format!("r/b@{D2}")).is_ok());
    }
}
