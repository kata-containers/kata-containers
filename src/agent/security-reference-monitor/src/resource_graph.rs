// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-4A — ordered, bijective resource graph.
//!
//! The container root filesystem is assembled from an ordered sequence of resources
//! (image layers, block devices, overlays). The weak enforcement pattern this replaces
//! only checks that *some* declared resource matches each presented resource and that
//! the counts are equal. That is exploitable in two ways:
//!
//!  - **Layer reorder** (Attack #4): presenting the declared layers in a different order
//!    still satisfies an existential/count check, yet produces a different root filesystem.
//!  - **Duplicate satisfies a declaration twice** (Attack #15 family): one declaration is
//!    matched by two presented resources.
//!
//! This module enforces an *order-relevant bijection*: the presented resources must equal
//! the declared resources one-for-one, in the same order, with each declaration bound to
//! exactly one presented resource and vice-versa, and each matched resource's integrity
//! digest (e.g. dm-verity root hash) must equal the declared value. A successful
//! verification yields typed [`VerifiedResourceHandle`]s that carry the declaration index,
//! so downstream code binds to the *verified* resource rather than a host-named alias.

/// Kind of a root-filesystem resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    /// An image layer (typically dm-verity protected).
    Layer,
    /// A block device (e.g. `blk`/`scsi`).
    BlockDevice,
    /// An overlay assembled in the guest.
    Overlay,
}

/// A declared resource: what the authorized policy says must be present, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDeclaration {
    pub kind: ResourceKind,
    /// Mount point / destination the resource is expected at.
    pub mount_point: String,
    /// Storage driver expected for the resource.
    pub driver: String,
    /// Expected integrity digest (dm-verity root hash or content digest).
    pub digest: String,
}

/// A presented resource: what the (untrusted) request actually offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentedResource {
    pub kind: ResourceKind,
    pub mount_point: String,
    pub driver: String,
    pub digest: String,
}

impl ResourceDeclaration {
    /// Identity fields (everything except the integrity digest). Two resources with the
    /// same identity but different digest are the "same slot, wrong content" case.
    fn identity(&self) -> (ResourceKind, &str, &str) {
        (self.kind, self.mount_point.as_str(), self.driver.as_str())
    }
}

impl PresentedResource {
    fn identity(&self) -> (ResourceKind, &str, &str) {
        (self.kind, self.mount_point.as_str(), self.driver.as_str())
    }
}

/// A resource that has been verified against its declaration and may be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedResourceHandle {
    /// Index of the declaration this resource was bound to (its position in the graph).
    pub declaration_index: usize,
    pub kind: ResourceKind,
    pub mount_point: String,
    pub digest: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResourceGraphError {
    /// The number of presented resources does not equal the number declared.
    CardinalityMismatch { declared: usize, presented: usize },
    /// A presented resource appears at a position whose declaration it does not match,
    /// but it matches a *different* declaration — i.e. the sequence was reordered.
    Reordered {
        position: usize,
        expected_mount: String,
        found_mount: String,
    },
    /// A presented resource matches no declaration at all.
    Undeclared {
        position: usize,
        found_mount: String,
    },
    /// The same resource identity is presented more than once (a duplicate that would
    /// otherwise satisfy a single declaration multiple times).
    Duplicate { mount_point: String },
    /// The resource is at the right position but its integrity digest does not match the
    /// declared value (e.g. a block device swapped for one with a stale dm-verity hash).
    DigestMismatch {
        position: usize,
        expected: String,
        found: String,
    },
}

impl std::fmt::Display for ResourceGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceGraphError::CardinalityMismatch { declared, presented } => write!(
                f,
                "resource cardinality mismatch: declared {declared}, presented {presented}"
            ),
            ResourceGraphError::Reordered {
                position,
                expected_mount,
                found_mount,
            } => write!(
                f,
                "resource reordered at position {position}: expected {expected_mount}, found {found_mount}"
            ),
            ResourceGraphError::Undeclared { position, found_mount } => {
                write!(f, "undeclared resource at position {position}: {found_mount}")
            }
            ResourceGraphError::Duplicate { mount_point } => {
                write!(f, "duplicate resource presented: {mount_point}")
            }
            ResourceGraphError::DigestMismatch {
                position,
                expected,
                found,
            } => write!(
                f,
                "integrity digest mismatch at position {position}: expected {expected}, found {found}"
            ),
        }
    }
}

impl std::error::Error for ResourceGraphError {}

/// Verify that `presented` is an order-relevant bijection of `declared`, with matching
/// integrity digests, and return the typed verified handles in declaration order.
///
/// The verification rejects: cardinality mismatch, reorder, undeclared/extra resources,
/// duplicates, and integrity (dm-verity) digest mismatches.
pub fn verify_ordered_bijection(
    declared: &[ResourceDeclaration],
    presented: &[PresentedResource],
) -> Result<Vec<VerifiedResourceHandle>, ResourceGraphError> {
    if declared.len() != presented.len() {
        return Err(ResourceGraphError::CardinalityMismatch {
            declared: declared.len(),
            presented: presented.len(),
        });
    }

    // Reject a resource identity presented more than once. A duplicate can never be part
    // of a bijection with distinct declarations, and detecting it explicitly gives a
    // precise error rather than surfacing as a downstream reorder.
    for i in 0..presented.len() {
        for j in (i + 1)..presented.len() {
            if presented[i].identity() == presented[j].identity() {
                return Err(ResourceGraphError::Duplicate {
                    mount_point: presented[i].mount_point.clone(),
                });
            }
        }
    }

    let mut handles = Vec::with_capacity(declared.len());
    for (position, (d, p)) in declared.iter().zip(presented.iter()).enumerate() {
        if p.identity() != d.identity() {
            // Wrong resource at this position: distinguish reorder from undeclared.
            let matches_elsewhere = declared.iter().any(|other| other.identity() == p.identity());
            return Err(if matches_elsewhere {
                ResourceGraphError::Reordered {
                    position,
                    expected_mount: d.mount_point.clone(),
                    found_mount: p.mount_point.clone(),
                }
            } else {
                ResourceGraphError::Undeclared {
                    position,
                    found_mount: p.mount_point.clone(),
                }
            });
        }
        if p.digest != d.digest {
            return Err(ResourceGraphError::DigestMismatch {
                position,
                expected: d.digest.clone(),
                found: p.digest.clone(),
            });
        }
        handles.push(VerifiedResourceHandle {
            declaration_index: position,
            kind: d.kind,
            mount_point: d.mount_point.clone(),
            digest: d.digest.clone(),
        });
    }

    Ok(handles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(kind: ResourceKind, mp: &str, driver: &str, digest: &str) -> ResourceDeclaration {
        ResourceDeclaration {
            kind,
            mount_point: mp.into(),
            driver: driver.into(),
            digest: digest.into(),
        }
    }
    fn pres(kind: ResourceKind, mp: &str, driver: &str, digest: &str) -> PresentedResource {
        PresentedResource {
            kind,
            mount_point: mp.into(),
            driver: driver.into(),
            digest: digest.into(),
        }
    }

    fn three_layers_decl() -> Vec<ResourceDeclaration> {
        vec![
            decl(ResourceKind::Layer, "/layer0", "blk", "verity-0"),
            decl(ResourceKind::Layer, "/layer1", "blk", "verity-1"),
            decl(ResourceKind::Layer, "/layer2", "blk", "verity-2"),
        ]
    }

    #[test]
    fn in_order_bijection_is_accepted() {
        let d = three_layers_decl();
        let p = vec![
            pres(ResourceKind::Layer, "/layer0", "blk", "verity-0"),
            pres(ResourceKind::Layer, "/layer1", "blk", "verity-1"),
            pres(ResourceKind::Layer, "/layer2", "blk", "verity-2"),
        ];
        let handles = verify_ordered_bijection(&d, &p).unwrap();
        assert_eq!(handles.len(), 3);
        assert_eq!(handles[2].declaration_index, 2);
        assert_eq!(handles[1].digest, "verity-1");
    }

    /// TC3.1: reordering image layers must be rejected.
    #[test]
    fn reordered_layers_are_rejected() {
        let d = three_layers_decl();
        let p = vec![
            pres(ResourceKind::Layer, "/layer1", "blk", "verity-1"),
            pres(ResourceKind::Layer, "/layer0", "blk", "verity-0"),
            pres(ResourceKind::Layer, "/layer2", "blk", "verity-2"),
        ];
        assert!(matches!(
            verify_ordered_bijection(&d, &p).unwrap_err(),
            ResourceGraphError::Reordered { position: 0, .. }
        ));
    }

    /// TC3.2: a duplicate layer that would match one declaration twice must be rejected.
    #[test]
    fn duplicate_layer_is_rejected() {
        let d = three_layers_decl();
        let p = vec![
            pres(ResourceKind::Layer, "/layer0", "blk", "verity-0"),
            pres(ResourceKind::Layer, "/layer0", "blk", "verity-0"),
            pres(ResourceKind::Layer, "/layer2", "blk", "verity-2"),
        ];
        assert_eq!(
            verify_ordered_bijection(&d, &p).unwrap_err(),
            ResourceGraphError::Duplicate {
                mount_point: "/layer0".into()
            }
        );
    }

    /// TC3.3a: a block device with a stale dm-verity hash must be rejected.
    #[test]
    fn stale_verity_hash_is_rejected() {
        let d = three_layers_decl();
        let p = vec![
            pres(ResourceKind::Layer, "/layer0", "blk", "verity-0"),
            pres(ResourceKind::Layer, "/layer1", "blk", "STALE"),
            pres(ResourceKind::Layer, "/layer2", "blk", "verity-2"),
        ];
        assert_eq!(
            verify_ordered_bijection(&d, &p).unwrap_err(),
            ResourceGraphError::DigestMismatch {
                position: 1,
                expected: "verity-1".into(),
                found: "STALE".into(),
            }
        );
    }

    /// TC3.3b: an undeclared / extra storage must be rejected.
    #[test]
    fn undeclared_resource_is_rejected() {
        let d = three_layers_decl();
        let p = vec![
            pres(ResourceKind::Layer, "/layer0", "blk", "verity-0"),
            pres(ResourceKind::Layer, "/layer1", "blk", "verity-1"),
            pres(ResourceKind::Layer, "/evil", "blk", "verity-x"),
        ];
        assert!(matches!(
            verify_ordered_bijection(&d, &p).unwrap_err(),
            ResourceGraphError::Undeclared { position: 2, .. }
        ));
    }

    #[test]
    fn extra_or_missing_resource_is_rejected() {
        let d = three_layers_decl();
        let mut p: Vec<_> = d
            .iter()
            .map(|x| pres(x.kind, &x.mount_point, &x.driver, &x.digest))
            .collect();
        p.push(pres(ResourceKind::Layer, "/layer3", "blk", "verity-3"));
        assert_eq!(
            verify_ordered_bijection(&d, &p).unwrap_err(),
            ResourceGraphError::CardinalityMismatch {
                declared: 3,
                presented: 4
            }
        );
    }

    /// TC3.4-style: a driver-option/driver mismatch at a position is caught (the identity
    /// includes the driver, so a swapped driver is a reorder-or-undeclared rejection).
    #[test]
    fn driver_mismatch_is_rejected() {
        let d = vec![decl(ResourceKind::Overlay, "/", "image_guest_pull", "d0")];
        let p = vec![pres(ResourceKind::Overlay, "/", "overlayfs", "d0")];
        assert!(matches!(
            verify_ordered_bijection(&d, &p).unwrap_err(),
            ResourceGraphError::Undeclared { position: 0, .. }
        ));
    }
}
