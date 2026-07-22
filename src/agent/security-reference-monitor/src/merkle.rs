// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! RFC 6962 Merkle tree — inclusion and consistency proofs.
//!
//! FR-1f Stage 2 anchors a policy fragment's transparency receipt in an append-only
//! transparency log (SCITT / Certificate-Transparency style). Two proofs are verified:
//!
//!  - an **inclusion proof** that the fragment statement is a leaf of the log at a given
//!    signed tree head (root + size) — i.e. the fragment was actually recorded; and
//!  - a **consistency proof** that each new signed tree head is an append-only extension of
//!    the previously-seen one — i.e. the log only ever grows and never rewrites history.
//!
//! Together with a monotonically non-decreasing, persisted tree head (see `FragmentStore`),
//! this proves *ordering*: fragments are anchored in a single, externally-auditable,
//! append-only log. The hashing is the standard RFC 6962 construction:
//!
//! ```text
//! leaf_hash(d)      = SHA-256(0x00 || d)
//! node_hash(l, r)   = SHA-256(0x01 || l || r)
//! ```
//!
//! Verification is pure-Rust (`sha2`); the tree builder here is used by tests, the offline
//! demo, and the mock-ledger dev tool to generate proofs.

use sha2::{Digest, Sha256};

/// RFC 6962 leaf hash: `SHA-256(0x00 || data)`.
pub fn leaf_hash(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([0x00]);
    h.update(data);
    h.finalize().into()
}

/// RFC 6962 interior node hash: `SHA-256(0x01 || left || right)`.
pub fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([0x01]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

/// Recompute the Merkle root from an inclusion proof for leaf `index` in a tree of `size`
/// leaves (RFC 6962 §2.1.1). Returns `None` if the proof is malformed (wrong length,
/// out-of-range index). The caller compares the result to the signed tree head root.
pub fn root_from_inclusion(
    index: u64,
    size: u64,
    leaf: [u8; 32],
    path: &[[u8; 32]],
) -> Option<[u8; 32]> {
    if index >= size {
        return None;
    }
    let mut idx = index;
    let mut last = size - 1;
    let mut r = leaf;
    let mut it = path.iter();
    while last != 0 {
        let p = it.next()?; // proof too short
        if idx & 1 == 1 || idx == last {
            r = node_hash(p, &r);
            if idx & 1 == 0 {
                // idx == last with idx even: climb to the next odd/zero level.
                while idx != 0 && idx & 1 == 0 {
                    idx >>= 1;
                    last >>= 1;
                }
            }
        } else {
            r = node_hash(&r, p);
        }
        idx >>= 1;
        last >>= 1;
    }
    if it.next().is_some() {
        return None; // proof too long
    }
    Some(r)
}

/// Verify an inclusion proof against a known root (convenience wrapper).
pub fn verify_inclusion(
    index: u64,
    size: u64,
    leaf: [u8; 32],
    path: &[[u8; 32]],
    root: &[u8; 32],
) -> bool {
    matches!(root_from_inclusion(index, size, leaf, path), Some(r) if &r == root)
}

/// Verify a consistency proof between an older tree head `(size1, root1)` and a newer one
/// `(size2, root2)` (RFC 6962 §2.1.2). Returns true iff the newer tree is an append-only
/// extension of the older one. Transcription of the canonical CT verifier.
pub fn verify_consistency(
    size1: u64,
    size2: u64,
    root1: &[u8; 32],
    root2: &[u8; 32],
    proof: &[[u8; 32]],
) -> bool {
    if size1 > size2 {
        return false;
    }
    if size1 == size2 {
        return proof.is_empty() && root1 == root2;
    }
    if size1 == 0 {
        // Any tree is consistent with the empty tree; the proof must be empty.
        return proof.is_empty();
    }

    let mut node = size1 - 1;
    let mut last_node = size2 - 1;
    while node & 1 == 1 {
        node >>= 1;
        last_node >>= 1;
    }

    let mut p = proof.iter();
    let (mut old_hash, mut new_hash) = if node != 0 {
        let seed = match p.next() {
            Some(h) => *h,
            None => return false,
        };
        (seed, seed)
    } else {
        // size1 is a power of two: the old root itself is the seed.
        (*root1, *root1)
    };

    while node != 0 {
        if node & 1 == 1 {
            let next = match p.next() {
                Some(h) => h,
                None => return false,
            };
            old_hash = node_hash(next, &old_hash);
            new_hash = node_hash(next, &new_hash);
        } else if node < last_node {
            let next = match p.next() {
                Some(h) => h,
                None => return false,
            };
            new_hash = node_hash(&new_hash, next);
        }
        node >>= 1;
        last_node >>= 1;
    }

    while last_node != 0 {
        let next = match p.next() {
            Some(h) => h,
            None => return false,
        };
        new_hash = node_hash(&new_hash, next);
        last_node >>= 1;
    }

    p.next().is_none() && &old_hash == root1 && &new_hash == root2
}

/// A simple in-memory RFC 6962 Merkle tree over a growing list of leaf entries. Used to
/// generate roots and proofs (tests, offline demo, mock-ledger dev tool). Not used on the
/// verification path — the agent only verifies proofs.
#[derive(Default, Clone)]
pub struct MerkleTree {
    leaves: Vec<Vec<u8>>,
}

impl MerkleTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: impl Into<Vec<u8>>) {
        self.leaves.push(entry.into());
    }

    pub fn size(&self) -> u64 {
        self.leaves.len() as u64
    }

    /// Merkle Tree Hash of the first `n` leaves (RFC 6962 §2.1).
    fn mth(&self, lo: usize, hi: usize) -> [u8; 32] {
        let n = hi - lo;
        if n == 0 {
            return Sha256::digest([]).into();
        }
        if n == 1 {
            return leaf_hash(&self.leaves[lo]);
        }
        let k = largest_pow2_below(n);
        node_hash(&self.mth(lo, lo + k), &self.mth(lo + k, hi))
    }

    /// Root over all current leaves.
    pub fn root(&self) -> [u8; 32] {
        self.mth(0, self.leaves.len())
    }

    /// Inclusion proof (audit path) for leaf `m` in the current tree.
    pub fn inclusion_proof(&self, m: usize) -> Vec<[u8; 32]> {
        self.path(m, 0, self.leaves.len())
    }

    fn path(&self, m: usize, lo: usize, hi: usize) -> Vec<[u8; 32]> {
        let n = hi - lo;
        if n <= 1 {
            return Vec::new();
        }
        let k = largest_pow2_below(n);
        if m < k {
            let mut pth = self.path(m, lo, lo + k);
            pth.push(self.mth(lo + k, hi));
            pth
        } else {
            let mut pth = self.path(m - k, lo + k, hi);
            pth.push(self.mth(lo, lo + k));
            pth
        }
    }

    /// Consistency proof between the first `m` leaves and the current tree (RFC 6962).
    pub fn consistency_proof(&self, m: usize) -> Vec<[u8; 32]> {
        self.subproof(m, 0, self.leaves.len(), true)
    }

    fn subproof(&self, m: usize, lo: usize, hi: usize, b: bool) -> Vec<[u8; 32]> {
        let n = hi - lo;
        if m == n {
            if b {
                return Vec::new();
            }
            return vec![self.mth(lo, hi)];
        }
        let k = largest_pow2_below(n);
        if m <= k {
            let mut pr = self.subproof(m, lo, lo + k, b);
            pr.push(self.mth(lo + k, hi));
            pr
        } else {
            let mut pr = self.subproof(m - k, lo + k, hi, false);
            pr.push(self.mth(lo, lo + k));
            pr
        }
    }
}

/// Largest power of two strictly less than `n` (for `n >= 2`).
fn largest_pow2_below(n: usize) -> usize {
    let mut k = 1;
    while k << 1 < n {
        k <<= 1;
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(n: usize) -> MerkleTree {
        let mut t = MerkleTree::new();
        for i in 0..n {
            t.push(format!("entry-{i}").into_bytes());
        }
        t
    }

    #[test]
    fn inclusion_proofs_verify_for_all_sizes_and_indices() {
        for n in 1..=33 {
            let t = tree(n);
            let root = t.root();
            for m in 0..n {
                let proof = t.inclusion_proof(m);
                let lh = leaf_hash(&t.leaves[m]);
                assert!(
                    verify_inclusion(m as u64, n as u64, lh, &proof, &root),
                    "inclusion n={} m={}", n, m
                );
                // A tampered leaf must not verify.
                let mut bad = lh;
                bad[0] ^= 0xff;
                assert!(!verify_inclusion(m as u64, n as u64, bad, &proof, &root));
            }
        }
    }

    #[test]
    fn consistency_proofs_verify_for_all_prefixes() {
        for n in 1..=33 {
            let t = tree(n);
            let root2 = t.root();
            for m in 1..=n {
                // Build the older tree (first m leaves) to get its root.
                let older = tree(m);
                let root1 = older.root();
                let proof = t.consistency_proof(m);
                assert!(
                    verify_consistency(m as u64, n as u64, &root1, &root2, &proof),
                    "consistency m={} n={}", m, n
                );
                // A wrong old root must not verify.
                let mut bad = root1;
                bad[0] ^= 0xff;
                assert!(!verify_consistency(m as u64, n as u64, &bad, &root2, &proof));
            }
        }
    }

    #[test]
    fn shrinking_tree_is_rejected() {
        let t = tree(8);
        let big = t.root();
        let small = tree(4).root();
        // Claiming the 8-leaf tree is a prefix of a 4-leaf tree is inconsistent.
        assert!(!verify_consistency(8, 4, &big, &small, &[]));
    }
}
