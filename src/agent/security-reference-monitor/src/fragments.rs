// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-1 — signed, add-only policy fragments.
//!
//! A base policy may be extended at runtime by *fragments* that grant additional,
//! narrowly-scoped capabilities (e.g. permitting a new container/process declaration).
//! To keep this safe, fragments are:
//!
//!  - **Signed** by an authorized issuer (Ed25519 over a canonical encoding); an unsigned
//!    or tampered fragment, or one from an unknown/unauthorized issuer, is rejected.
//!  - **Monotonic** per issuer: each accepted fragment must carry a strictly increasing
//!    security version number (SVN); a replayed or rolled-back SVN is rejected.
//!  - **Add-only and fail-closed**: a fragment may only *add* grants. It may never
//!    introduce a grant that relaxes a declared root constraint (an invariant of the base
//!    policy); such a fragment is rejected outright rather than partially applied.
//!  - **Transparency-backed** (optional, enabled in strict mode): a fragment must carry a
//!    transparency receipt so its issuance is auditable. (The receipt's cryptographic
//!    verification against a transparency service is a separate, environment-specific
//!    step; here its presence is required and its identifier is bound into the signature.)
//!
//! Verification is performed with a maintained pure-Rust Ed25519 verifier; no Go
//! dependency is introduced into the agent.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// The ledger id used for a receipt that does not name one, and for the back-compat
/// single-anchor configuration (`set_transparency_anchor`).
pub const DEFAULT_LEDGER: &str = "default";

/// A policy fragment presented for loading.
#[derive(Debug, Clone, Default)]
pub struct PolicyFragment {
    /// Identifier of the issuer that signed the fragment.
    pub issuer: String,
    /// FR-1e: the logical feed (scope) under the issuer. The base policy declares which
    /// `(issuer, feed)` pairs it accepts and their SVN floor. Empty = the default feed.
    pub feed: String,
    /// Security version number; must strictly increase per `(issuer, feed)`.
    pub svn: u64,
    /// Additional grants introduced by this fragment (add-only). Legacy/opaque form.
    pub grants: Vec<String>,
    /// FR-1c: a signed Rego module (text) the fragment contributes to the policy engine
    /// (Model A). Must declare a package under the reserved fragment namespace.
    pub policy_module: Option<String>,
    /// FR-1c: the policy namespaces this fragment is scoped to contribute to. The applier
    /// refuses a module whose package is outside these.
    pub includes: Vec<String>,
    /// FR-1g: identifiers of fragments that must already be loaded before this one
    /// (composition). A fragment id is `"<issuer>/<feed>/<svn>"`.
    pub requires: Vec<String>,
    /// FR-1f: transparency receipt — a detached signature (hex) by a transparency ledger
    /// key over [`PolicyFragment::signing_bytes`]. Required when receipts are enforced;
    /// verified against a measured trust list when one is configured.
    pub receipt: Option<String>,
    /// FR-1f (trust list): the transparency ledger this receipt is claimed to originate
    /// from. Selects which ledger's key(s) the receipt is verified against and is subject to
    /// `allowed_ledgers`/`required_receipt_from` scoping. `None`/empty = the default ledger
    /// (back-compat with a single-anchor configuration). This is *untrusted* metadata: it
    /// only selects a key set; a receipt is accepted only if that ledger's key actually
    /// signed the statement, so a forged ledger id cannot bypass verification.
    pub receipt_ledger: Option<String>,
    /// Detached Ed25519 signature (by the issuer) over [`PolicyFragment::signing_bytes`].
    pub signature: Vec<u8>,
}

impl PolicyFragment {
    /// This fragment's composition identifier: `"<issuer>/<feed>/<svn>"`.
    pub fn id(&self) -> String {
        format!("{}/{}/{}", self.issuer, self.feed, self.svn)
    }

    /// Canonical byte encoding of the fragment *statement* that both the issuer signature
    /// and the transparency receipt cover. Deterministic and binds issuer, feed, SVN,
    /// sorted grants, module, sorted includes, and sorted requires — so none can be altered
    /// without invalidating the signature. The receipt itself is NOT included (it is a
    /// separate signature over these same bytes, created after the issuer signs).
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut grants = self.grants.clone();
        grants.sort();
        let mut includes = self.includes.clone();
        includes.sort();
        let mut requires = self.requires.clone();
        requires.sort();
        let mut s = String::new();
        s.push_str("kata-policy-fragment/v2\n");
        s.push_str(&self.issuer);
        s.push('\n');
        s.push_str(&self.feed);
        s.push('\n');
        s.push_str(&self.svn.to_string());
        s.push('\n');
        for g in &grants {
            s.push_str(g);
            s.push('\n');
        }
        s.push_str("--includes--\n");
        for i in &includes {
            s.push_str(i);
            s.push('\n');
        }
        s.push_str("--requires--\n");
        for r in &requires {
            s.push_str(r);
            s.push('\n');
        }
        s.push_str("--module--\n");
        s.push_str(self.policy_module.as_deref().unwrap_or(""));
        s.into_bytes()
    }
}

/// A fragment that has passed every verification gate but has not yet been committed to the
/// store. Returned by [`FragmentStore::verify`] so the caller can apply it to the policy
/// engine and only then [`FragmentStore::commit`] it — keeping verify+apply atomic.
#[derive(Debug, Clone)]
pub struct VerifiedFragment {
    pub issuer: String,
    pub feed: String,
    pub svn: u64,
    pub id: String,
    pub grants: Vec<String>,
    pub policy_module: Option<String>,
    pub includes: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FragmentError {
    /// The fragment's issuer is not in the authorized-issuer set.
    UnauthorizedIssuer(String),
    /// The signature is malformed, or does not verify under the issuer's key (covers the
    /// unsigned case: an empty/garbage signature).
    InvalidSignature,
    /// The SVN is not strictly greater than the last accepted SVN for this issuer.
    RolledBackSvn {
        issuer: String,
        presented: u64,
        min_required: u64,
    },
    /// Receipts are enforced but the fragment carries none.
    MissingReceipt,
    /// FR-1f: the transparency receipt does not verify against the configured trust list.
    InvalidReceipt,
    /// FR-1f (trust list): the receipt's ledger is not in the `allowed_ledgers` scope for
    /// this `(issuer, feed)`.
    LedgerNotAllowed { issuer: String, feed: String, ledger: String },
    /// FR-1f (trust list): a receipt is required from a specific ledger for this scope, but
    /// the presented receipt originates from a different (or unspecified) ledger.
    ReceiptFromDisallowedLedger { required: String, presented: String },
    /// FR-1e: the fragment's `(issuer, feed)` pair is not declared/accepted.
    UndeclaredFeed { issuer: String, feed: String },
    /// FR-1g: a required (dependency) fragment has not been loaded.
    UnsatisfiedRequirement { requires: String },
    /// The fragment introduces a grant that would relax a declared root constraint.
    RootConstraintRelaxation(String),
}

impl fmt::Display for FragmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FragmentError::UnauthorizedIssuer(i) => write!(f, "unauthorized fragment issuer: {i}"),
            FragmentError::InvalidSignature => write!(f, "invalid fragment signature"),
            FragmentError::RolledBackSvn {
                issuer,
                presented,
                min_required,
            } => write!(
                f,
                "rolled-back SVN for issuer {issuer}: presented {presented}, require >= {min_required}"
            ),
            FragmentError::MissingReceipt => write!(f, "fragment is missing a transparency receipt"),
            FragmentError::InvalidReceipt => write!(f, "fragment transparency receipt is invalid"),
            FragmentError::LedgerNotAllowed { issuer, feed, ledger } => write!(
                f,
                "receipt ledger {ledger:?} not allowed for issuer {issuer}, feed {feed:?}"
            ),
            FragmentError::ReceiptFromDisallowedLedger { required, presented } => write!(
                f,
                "receipt required from ledger {required:?}, but presented from {presented:?}"
            ),
            FragmentError::UndeclaredFeed { issuer, feed } => {
                write!(f, "undeclared fragment feed: issuer {issuer}, feed {feed:?}")
            }
            FragmentError::UnsatisfiedRequirement { requires } => {
                write!(f, "required fragment not loaded: {requires}")
            }
            FragmentError::RootConstraintRelaxation(g) => {
                write!(f, "fragment would relax a root constraint: {g}")
            }
        }
    }
}

impl std::error::Error for FragmentError {}

/// Verifier and add-only accumulator for policy fragments.
#[derive(Default)]
pub struct FragmentStore {
    issuers: HashMap<String, VerifyingKey>,
    /// Monotonic SVN high-water mark keyed by (issuer, feed).
    last_svn: HashMap<(String, String), u64>,
    /// FR-1e: declared/accepted (issuer, feed) pairs and their SVN floor.
    feeds: HashMap<(String, String), u64>,
    root_constraints: HashSet<String>,
    require_receipt: bool,
    active_grants: HashSet<String>,
    /// FR-1f (trust list): the Transparency Trust List — a map of ledger id to that
    /// ledger's current verification key(s). Multiple keys per ledger support rotation: a
    /// receipt verifies if *any* current key of the named ledger validates it. When
    /// non-empty, a fragment's receipt is cryptographically verified against the selected
    /// ledger's keys.
    transparency_trust_list: HashMap<String, Vec<VerifyingKey>>,
    /// FR-1f (trust list): per-`(issuer, feed)` allow-list of ledger ids. A receipt whose
    /// ledger is not in this list (when the list is non-empty) is rejected.
    allowed_ledgers: HashMap<(String, String), Vec<String>>,
    /// FR-1f (trust list): per-`(issuer, feed)` policy-driven receipt requirement — the set
    /// of ledgers a receipt must originate from for this scope. Non-empty ⇒ a receipt from
    /// one of these ledgers is mandatory (overrides the global `require_receipt` default).
    required_receipt_from: HashMap<(String, String), Vec<String>>,
    /// FR-1g: identifiers of fragments that have been loaded (for composition).
    loaded_ids: HashSet<String>,
}

impl FragmentStore {
    /// Create a store. `require_receipt` should be true in strict mode.
    pub fn new(require_receipt: bool) -> Self {
        Self {
            require_receipt,
            ..Default::default()
        }
    }

    /// Authorize an issuer by its 32-byte Ed25519 public key. Returns an error if the key
    /// is not a valid Ed25519 public key.
    pub fn authorize_issuer(
        &mut self,
        issuer: impl Into<String>,
        public_key: &[u8; 32],
    ) -> Result<(), FragmentError> {
        let key = VerifyingKey::from_bytes(public_key).map_err(|_| FragmentError::InvalidSignature)?;
        let issuer = issuer.into();
        // Authorizing an issuer also declares its default feed ("") so simple fragments
        // (no explicit feed) are accepted; named feeds are added via `declare_feed`.
        self.feeds.entry((issuer.clone(), String::new())).or_insert(0);
        self.issuers.insert(issuer, key);
        Ok(())
    }

    /// FR-1b: set a declarative minimum-SVN floor for an issuer's default feed (from
    /// measured state). A fragment is only accepted at `svn >= floor`.
    pub fn set_min_svn(&mut self, issuer: impl Into<String>, floor: u64) {
        self.feeds.insert((issuer.into(), String::new()), floor);
    }

    /// FR-1e: declare an accepted `(issuer, feed)` pair with its SVN floor (from measured
    /// state). A fragment whose `(issuer, feed)` is not declared is rejected.
    pub fn declare_feed(
        &mut self,
        issuer: impl Into<String>,
        feed: impl Into<String>,
        min_svn: u64,
    ) {
        self.feeds.insert((issuer.into(), feed.into()), min_svn);
    }

    /// FR-1f (trust list): load the Transparency Trust List — a set of `(ledger_id, keys)`
    /// entries from measured state. Each ledger may carry multiple keys to support rotation
    /// (a receipt verifies against any current key). Invalid keys are rejected.
    pub fn load_transparency_trust_list(
        &mut self,
        entries: &[(String, Vec<[u8; 32]>)],
    ) -> Result<(), FragmentError> {
        for (id, keys) in entries {
            let mut vks = Vec::with_capacity(keys.len());
            for k in keys {
                vks.push(VerifyingKey::from_bytes(k).map_err(|_| FragmentError::InvalidSignature)?);
            }
            self.transparency_trust_list
                .entry(id.clone())
                .or_default()
                .extend(vks);
        }
        Ok(())
    }

    /// FR-1f (trust list): scope the ledgers a receipt may originate from for a given
    /// `(issuer, feed)`. A non-empty list rejects receipts from any other ledger.
    pub fn set_allowed_ledgers(
        &mut self,
        issuer: impl Into<String>,
        feed: impl Into<String>,
        ledgers: &[String],
    ) {
        self.allowed_ledgers
            .insert((issuer.into(), feed.into()), ledgers.to_vec());
    }

    /// FR-1f (trust list): policy-driven `required_receipts` — require a receipt from one of
    /// `from_ledgers` for this `(issuer, feed)`. A non-empty list makes a receipt mandatory
    /// for the scope (overriding the global default) and constrains its ledger.
    pub fn require_receipt_for(
        &mut self,
        issuer: impl Into<String>,
        feed: impl Into<String>,
        from_ledgers: &[String],
    ) {
        self.required_receipt_from
            .insert((issuer.into(), feed.into()), from_ledgers.to_vec());
    }

    /// FR-1f: set a single transparency anchor public key. Back-compat shim over the trust
    /// list: registers the key under the default ledger. When set, a fragment's receipt is
    /// cryptographically verified against it (a detached signature over the fragment
    /// statement); without any trust-list entry, receipts are only checked for presence.
    pub fn set_transparency_anchor(&mut self, public_key: &[u8; 32]) -> Result<(), FragmentError> {
        self.load_transparency_trust_list(&[(DEFAULT_LEDGER.to_string(), vec![*public_key])])
    }

    /// Declare a grant that no fragment may ever introduce (a base-policy invariant).
    pub fn add_root_constraint(&mut self, grant: impl Into<String>) {
        self.root_constraints.insert(grant.into());
    }

    /// The set of grants currently active from all accepted fragments.
    pub fn active_grants(&self) -> &HashSet<String> {
        &self.active_grants
    }

    /// Whether any issuer is authorized (fail-closed indicator).
    pub fn has_authorized_issuers(&self) -> bool {
        !self.issuers.is_empty()
    }

    /// The minimum SVN the next fragment for `(issuer, feed)` must carry (declarative floor
    /// combined with the monotonic high-water mark of accepted fragments).
    fn min_required(&self, issuer: &str, feed: &str) -> u64 {
        let key = (issuer.to_string(), feed.to_string());
        let floor = self.feeds.get(&key).copied().unwrap_or(0);
        match self.last_svn.get(&key) {
            Some(last) => (last + 1).max(floor),
            None => floor,
        }
    }

    /// Verify a fragment against every gate **without** mutating the store. Returns the
    /// verified fragment so the caller can apply it to the policy engine and only then
    /// [`commit`](Self::commit) it. On any failure the store is unchanged (fail-closed).
    pub fn verify(&self, fragment: &PolicyFragment) -> Result<VerifiedFragment, FragmentError> {
        // 1. Issuer must be authorized.
        let key = self
            .issuers
            .get(&fragment.issuer)
            .ok_or_else(|| FragmentError::UnauthorizedIssuer(fragment.issuer.clone()))?;

        // 2. Issuer signature must verify over the fragment statement (rejects
        //    unsigned/tampered; the statement binds issuer/feed/svn/grants/includes/
        //    requires/module).
        let statement = fragment.signing_bytes();
        let sig =
            Signature::from_slice(&fragment.signature).map_err(|_| FragmentError::InvalidSignature)?;
        key.verify(&statement, &sig)
            .map_err(|_| FragmentError::InvalidSignature)?;

        // 3+. Remaining gates (feed, SVN, receipt, requires, add-only).
        self.check_gates(fragment, &statement)
    }

    /// FR-1h: verify a fragment carried in a COSE_Sign1 (CBOR) envelope, for interop with
    /// COSE tooling (`sign1util` / `az confcom`). The envelope's payload must equal the
    /// fragment statement, and its signature must verify against the authorized issuer's
    /// Ed25519 key (EdDSA). After signature verification the same gates as [`verify`] apply.
    pub fn verify_cose(
        &self,
        fragment: &PolicyFragment,
        cose_sign1: &[u8],
    ) -> Result<VerifiedFragment, FragmentError> {
        let key = self
            .issuers
            .get(&fragment.issuer)
            .ok_or_else(|| FragmentError::UnauthorizedIssuer(fragment.issuer.clone()))?;

        use coset::CborSerializable;
        let sign1 = coset::CoseSign1::from_slice(cose_sign1)
            .map_err(|_| FragmentError::InvalidSignature)?;

        // The COSE payload must be exactly the fragment statement, binding the envelope to
        // the presented fields.
        let statement = fragment.signing_bytes();
        match &sign1.payload {
            Some(p) if p.as_slice() == statement.as_slice() => {}
            _ => return Err(FragmentError::InvalidSignature),
        }

        // Verify the COSE_Sign1 signature (coset reconstructs the Sig_structure).
        sign1
            .verify_signature(b"", |sig, tbs| {
                let s = Signature::from_slice(sig).map_err(|_| ())?;
                key.verify(tbs, &s).map_err(|_| ())
            })
            .map_err(|_| FragmentError::InvalidSignature)?;

        self.check_gates(fragment, &statement)
    }

    /// The verification gates that follow signature verification (shared by the native and
    /// COSE paths): feed declared, monotonic SVN, transparency receipt, requires loaded,
    /// add-only.
    fn check_gates(
        &self,
        fragment: &PolicyFragment,
        statement: &[u8],
    ) -> Result<VerifiedFragment, FragmentError> {
        // 3. FR-1e: the (issuer, feed) pair must be declared/accepted.
        let feed_key = (fragment.issuer.clone(), fragment.feed.clone());
        if !self.feeds.contains_key(&feed_key) {
            return Err(FragmentError::UndeclaredFeed {
                issuer: fragment.issuer.clone(),
                feed: fragment.feed.clone(),
            });
        }

        // 4. Monotonic SVN per (issuer, feed): >= the declared floor and strictly greater
        //    than the last accepted.
        let min_required = self.min_required(&fragment.issuer, &fragment.feed);
        if fragment.svn < min_required {
            return Err(FragmentError::RolledBackSvn {
                issuer: fragment.issuer.clone(),
                presented: fragment.svn,
                min_required,
            });
        }

        // 5. Transparency receipt (FR-1f trust list): a receipt may be required globally or
        //    per-scope (`required_receipt_from`); when present it must originate from an
        //    allowed ledger and verify against one of that ledger's current keys.
        let receipt = fragment.receipt.as_deref().unwrap_or("");
        let ledger = fragment
            .receipt_ledger
            .as_deref()
            .filter(|l| !l.is_empty())
            .unwrap_or(DEFAULT_LEDGER);

        // Per-scope required ledgers (policy-driven `required_receipts`). A non-empty list
        // makes a receipt mandatory for this scope and constrains its ledger.
        let required = self.required_receipt_from.get(&feed_key);
        let scope_requires = required.map(|r| !r.is_empty()).unwrap_or(false);

        if receipt.is_empty() {
            if scope_requires || self.require_receipt {
                return Err(FragmentError::MissingReceipt);
            }
        } else {
            // The presented ledger must be in the scope's allow-list (if one is set).
            if let Some(allowed) = self.allowed_ledgers.get(&feed_key) {
                if !allowed.is_empty() && !allowed.iter().any(|l| l == ledger) {
                    return Err(FragmentError::LedgerNotAllowed {
                        issuer: fragment.issuer.clone(),
                        feed: fragment.feed.clone(),
                        ledger: ledger.to_string(),
                    });
                }
            }
            // If the scope requires a receipt from a specific ledger, enforce it.
            if let Some(req_ledgers) = required {
                if !req_ledgers.is_empty() && !req_ledgers.iter().any(|l| l == ledger) {
                    return Err(FragmentError::ReceiptFromDisallowedLedger {
                        required: req_ledgers.join(","),
                        presented: ledger.to_string(),
                    });
                }
            }
            // Cryptographic verification against the named ledger's key(s), when a trust
            // list is configured. Any current key of that ledger may validate the receipt
            // (rotation). An unknown ledger id has no keys ⇒ InvalidReceipt.
            if !self.transparency_trust_list.is_empty() {
                let bytes = hex_to_bytes(receipt).map_err(|_| FragmentError::InvalidReceipt)?;
                let rsig = Signature::from_slice(&bytes).map_err(|_| FragmentError::InvalidReceipt)?;
                let keys = self
                    .transparency_trust_list
                    .get(ledger)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                if !keys.iter().any(|k| k.verify(statement, &rsig).is_ok()) {
                    return Err(FragmentError::InvalidReceipt);
                }
            }
        }

        // 6. FR-1g: every required (dependency) fragment must already be loaded.
        for req in &fragment.requires {
            if !self.loaded_ids.contains(req) {
                return Err(FragmentError::UnsatisfiedRequirement {
                    requires: req.clone(),
                });
            }
        }

        // 7. Add-only: reject any grant that would relax a root constraint.
        for g in &fragment.grants {
            if self.root_constraints.contains(g) {
                return Err(FragmentError::RootConstraintRelaxation(g.clone()));
            }
        }

        Ok(VerifiedFragment {
            issuer: fragment.issuer.clone(),
            feed: fragment.feed.clone(),
            svn: fragment.svn,
            id: fragment.id(),
            grants: fragment.grants.clone(),
            policy_module: fragment.policy_module.clone(),
            includes: fragment.includes.clone(),
        })
    }

    /// Commit a previously [`verify`](Self::verify)-ed fragment: advance the `(issuer, feed)`
    /// SVN high-water mark, record the fragment id (for composition), and accumulate its
    /// grants. Returns the grants newly added.
    pub fn commit(&mut self, verified: &VerifiedFragment) -> Vec<String> {
        self.last_svn
            .insert((verified.issuer.clone(), verified.feed.clone()), verified.svn);
        self.loaded_ids.insert(verified.id.clone());
        let mut added = Vec::new();
        for g in &verified.grants {
            if self.active_grants.insert(g.clone()) {
                added.push(g.clone());
            }
        }
        added
    }

    /// Verify and commit a fragment in one step (verify + commit). On any failure the store
    /// is left unchanged (fail-closed). Returns the grants newly added.
    pub fn load(&mut self, fragment: &PolicyFragment) -> Result<Vec<String>, FragmentError> {
        let verified = self.verify(fragment)?;
        Ok(self.commit(&verified))
    }

    /// FR-1i: export the per-`(issuer, feed)` SVN high-water marks as a stable text snapshot
    /// (`issuer\tfeed\tsvn` per line) for persistence to a sealed/measured store. Sorted
    /// for determinism.
    pub fn export_svn_state(&self) -> String {
        let mut lines: Vec<String> = self
            .last_svn
            .iter()
            .map(|((issuer, feed), svn)| format!("{issuer}\t{feed}\t{svn}"))
            .collect();
        lines.sort();
        lines.join("\n")
    }

    /// FR-1i: import a persisted SVN snapshot on boot. Each entry can only **raise** the
    /// high-water mark for its `(issuer, feed)`, never lower it — so an agent/VM restart can
    /// never reopen a rollback window (a fragment at or below a previously-accepted SVN
    /// stays rejected). Combined with the declarative floor (FR-1e), the effective minimum
    /// is `max(declared floor, persisted high-water + 1)`.
    pub fn import_svn_state(&mut self, snapshot: &str) {
        for line in snapshot.lines() {
            let mut it = line.splitn(3, '\t');
            let (Some(issuer), Some(feed), Some(svn)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            let Ok(svn) = svn.trim().parse::<u64>() else {
                continue;
            };
            let key = (issuer.to_string(), feed.to_string());
            let entry = self.last_svn.entry(key).or_insert(0);
            *entry = (*entry).max(svn);
        }
    }
}

/// Decode a hex string into bytes.
fn hex_to_bytes(s: &str) -> Result<Vec<u8>, ()> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn keypair(seed: u8) -> (SigningKey, [u8; 32]) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let pk = sk.verifying_key().to_bytes();
        (sk, pk)
    }

    fn sign(sk: &SigningKey, f: &mut PolicyFragment) {
        f.signature = sk.sign(&f.signing_bytes()).to_bytes().to_vec();
    }

    fn frag(issuer: &str, svn: u64, grants: &[&str]) -> PolicyFragment {
        PolicyFragment {
            issuer: issuer.to_string(),
            svn,
            grants: grants.iter().map(|s| s.to_string()).collect(),
            receipt: Some("receipt-1".to_string()),
            signature: Vec::new(),
            ..Default::default()
        }
    }

    /// TC-F1.5: a declarative minimum-SVN floor (from measured state) is enforced — a
    /// fragment below the floor is rejected, one at/above the floor is accepted.
    #[test]
    fn min_svn_floor_is_enforced() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();
        store.set_min_svn("issuerA", 5);

        let mut below = frag("issuerA", 4, &["exec:x"]);
        sign(&sk, &mut below);
        assert!(matches!(
            store.load(&below).unwrap_err(),
            FragmentError::RolledBackSvn { min_required: 5, .. }
        ));

        let mut at_floor = frag("issuerA", 5, &["exec:x"]);
        sign(&sk, &mut at_floor);
        assert!(store.load(&at_floor).is_ok());
    }

    /// TC-F1.6: with no authorized issuers (absent measured config), every fragment is
    /// rejected — fail-closed.
    #[test]
    fn no_authorized_issuers_is_fail_closed() {
        let (sk, _pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        assert!(!store.has_authorized_issuers());
        let mut f = frag("issuerA", 1, &["exec:x"]);
        sign(&sk, &mut f);
        assert_eq!(
            store.load(&f).unwrap_err(),
            FragmentError::UnauthorizedIssuer("issuerA".into())
        );
    }

    /// TC-F1.7 / TC-F1.8: the contributed Rego module and the `includes` scope are bound
    /// into the signature — mutating either invalidates it.
    #[test]
    fn module_and_includes_are_signature_bound() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();

        let mut f = PolicyFragment {
            issuer: "issuerA".into(),
            svn: 1,
            grants: vec![],
            policy_module: Some("package agent_policy.fragments\nexec_allowed := true".into()),
            includes: vec!["exec".into()],
            receipt: Some("r".into()),
            signature: Vec::new(),
            ..Default::default()
        };
        sign(&sk, &mut f);
        let v = store.verify(&f).unwrap();
        assert_eq!(v.includes, vec!["exec".to_string()]);
        assert!(v.policy_module.is_some());

        // Tamper the module after signing → signature no longer verifies.
        let mut tampered = f.clone();
        tampered.policy_module = Some("package agent_policy.fragments\nexec_allowed := true # evil".into());
        assert_eq!(
            store.verify(&tampered).unwrap_err(),
            FragmentError::InvalidSignature
        );

        // Tamper the includes after signing → signature no longer verifies.
        let mut tampered2 = f.clone();
        tampered2.includes = vec!["mount".into()];
        assert_eq!(
            store.verify(&tampered2).unwrap_err(),
            FragmentError::InvalidSignature
        );
    }

    /// TC-F1.4 (store half): verify does not mutate; commit does — supporting atomic
    /// verify→apply→commit at the call site.
    #[test]
    fn verify_is_side_effect_free_until_commit() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();
        let mut f = frag("issuerA", 7, &["exec:x"]);
        sign(&sk, &mut f);

        let v = store.verify(&f).unwrap();
        // Not committed yet: no grants, SVN not advanced (a re-verify still succeeds).
        assert!(store.active_grants().is_empty());
        assert!(store.verify(&f).is_ok());

        store.commit(&v);
        assert!(store.active_grants().contains("exec:x"));
        // After commit the same SVN is now a rollback.
        assert!(matches!(
            store.verify(&f).unwrap_err(),
            FragmentError::RolledBackSvn { .. }
        ));
    }


    /// TC4.8: a valid signed add-only fragment (with receipt) is accepted and its grants
    /// become active.
    #[test]
    fn valid_signed_fragment_is_accepted() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();
        let mut f = frag("issuerA", 1, &["exec:container-x"]);
        sign(&sk, &mut f);
        let added = store.load(&f).unwrap();
        assert_eq!(added, vec!["exec:container-x".to_string()]);
        assert!(store.active_grants().contains("exec:container-x"));
    }

    /// TC4.4: an unsigned fragment (empty/garbage signature) is rejected.
    #[test]
    fn unsigned_fragment_is_rejected() {
        let (_sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();
        let f = frag("issuerA", 1, &["exec:x"]); // signature left empty
        assert_eq!(store.load(&f).unwrap_err(), FragmentError::InvalidSignature);
    }

    /// TC4.5: a fragment from an unauthorized issuer is rejected.
    #[test]
    fn unauthorized_issuer_is_rejected() {
        let (sk, _pk) = keypair(1);
        let store_pk = keypair(2).1; // a different, authorized key
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &store_pk).unwrap();
        // Fragment claims issuerA but is signed with the wrong key.
        let mut f = frag("issuerA", 1, &["exec:x"]);
        sign(&sk, &mut f);
        assert_eq!(store.load(&f).unwrap_err(), FragmentError::InvalidSignature);

        // Fragment from a completely unknown issuer.
        let mut g = frag("issuerB", 1, &["exec:x"]);
        sign(&sk, &mut g);
        assert_eq!(
            store.load(&g).unwrap_err(),
            FragmentError::UnauthorizedIssuer("issuerB".into())
        );
    }

    /// TC4.6: a rolled-back / replayed SVN is rejected (monotonic SVN).
    #[test]
    fn rolled_back_svn_is_rejected() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();

        let mut f5 = frag("issuerA", 5, &["exec:x"]);
        sign(&sk, &mut f5);
        store.load(&f5).unwrap();

        // Replay same SVN.
        let mut f5b = frag("issuerA", 5, &["exec:y"]);
        sign(&sk, &mut f5b);
        assert!(matches!(
            store.load(&f5b).unwrap_err(),
            FragmentError::RolledBackSvn { .. }
        ));

        // Older SVN.
        let mut f3 = frag("issuerA", 3, &["exec:z"]);
        sign(&sk, &mut f3);
        assert!(matches!(
            store.load(&f3).unwrap_err(),
            FragmentError::RolledBackSvn { .. }
        ));

        // Strictly greater SVN is accepted.
        let mut f6 = frag("issuerA", 6, &["exec:w"]);
        sign(&sk, &mut f6);
        assert!(store.load(&f6).is_ok());
    }

    /// TC4.7: a fragment that tries to introduce a grant relaxing a root constraint is
    /// rejected (add-only, fail-closed).
    #[test]
    fn over_broad_fragment_is_rejected() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();
        store.add_root_constraint("allow-all");
        let mut f = frag("issuerA", 1, &["exec:x", "allow-all"]);
        sign(&sk, &mut f);
        assert_eq!(
            store.load(&f).unwrap_err(),
            FragmentError::RootConstraintRelaxation("allow-all".into())
        );
        // Fail-closed: nothing from the rejected fragment was applied.
        assert!(store.active_grants().is_empty());
    }

    /// A fragment with no transparency receipt is rejected when receipts are enforced.
    #[test]
    fn missing_receipt_is_rejected_in_strict() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();
        let mut f = frag("issuerA", 1, &["exec:x"]);
        f.receipt = None;
        sign(&sk, &mut f);
        assert_eq!(store.load(&f).unwrap_err(), FragmentError::MissingReceipt);
    }

    fn frag_feed(issuer: &str, feed: &str, svn: u64) -> PolicyFragment {
        PolicyFragment {
            issuer: issuer.to_string(),
            feed: feed.to_string(),
            svn,
            receipt: Some("receipt-1".to_string()),
            ..Default::default()
        }
    }

    /// Helper: build a ledger-signed receipt (hex) over a fragment's statement, tag its
    /// ledger id, and issuer-sign the fragment.
    fn signed_with_receipt(
        issuer_sk: &SigningKey,
        f: &mut PolicyFragment,
        ledger: &str,
        ledger_sk: &SigningKey,
    ) {
        f.signature = issuer_sk.sign(&f.signing_bytes()).to_bytes().to_vec();
        let rsig = ledger_sk.sign(&f.signing_bytes());
        f.receipt = Some(rsig.to_bytes().iter().map(|b| format!("{:02x}", b)).collect());
        f.receipt_ledger = Some(ledger.to_string());
    }

    /// TC-F1.22 (FR-1f trust list): a multi-ledger trust list accepts a receipt from an
    /// allowed ledger whose key signed the statement.
    #[test]
    fn trust_list_accepts_allowed_ledger() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (ledger_a_sk, ledger_a_pk) = keypair(20);
        let (_ledger_b_sk, ledger_b_pk) = keypair(21);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store
            .load_transparency_trust_list(&[
                ("ledgerA".into(), vec![ledger_a_pk]),
                ("ledgerB".into(), vec![ledger_b_pk]),
            ])
            .unwrap();

        let mut f = frag_feed("issuerA", "", 1);
        signed_with_receipt(&issuer_sk, &mut f, "ledgerA", &ledger_a_sk);
        assert!(store.verify(&f).is_ok());

        // A receipt tagged for ledgerB but signed by ledgerA's key does not verify against
        // ledgerB's key -> InvalidReceipt (a forged ledger id cannot bypass verification).
        f.receipt_ledger = Some("ledgerB".into());
        assert_eq!(store.verify(&f).unwrap_err(), FragmentError::InvalidReceipt);
    }

    /// TC-F1.23 (FR-1f trust list): a receipt from a ledger outside the scope's
    /// `allowed_ledgers` is rejected even if its signature is otherwise valid.
    #[test]
    fn trust_list_rejects_disallowed_ledger() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (ledger_a_sk, ledger_a_pk) = keypair(20);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store
            .load_transparency_trust_list(&[("ledgerA".into(), vec![ledger_a_pk])])
            .unwrap();
        // Only ledgerB is allowed for the default feed, but the receipt is from ledgerA.
        store.set_allowed_ledgers("issuerA", "", &["ledgerB".to_string()]);

        let mut f = frag_feed("issuerA", "", 1);
        signed_with_receipt(&issuer_sk, &mut f, "ledgerA", &ledger_a_sk);
        assert_eq!(
            store.verify(&f).unwrap_err(),
            FragmentError::LedgerNotAllowed {
                issuer: "issuerA".into(),
                feed: "".into(),
                ledger: "ledgerA".into(),
            }
        );
    }

    /// TC-F1.24 (FR-1f trust list): policy-driven `required_receipts` per feed — feed
    /// "prod" requires a receipt from a specific ledger; feed "dev" requires none.
    #[test]
    fn per_feed_required_receipts_enforced() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (ledger_a_sk, ledger_a_pk) = keypair(20);
        // Global receipt requirement off; requirement is expressed per-scope instead.
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store.declare_feed("issuerA", "prod", 0);
        store.declare_feed("issuerA", "dev", 0);
        store
            .load_transparency_trust_list(&[("ledgerA".into(), vec![ledger_a_pk])])
            .unwrap();
        store.require_receipt_for("issuerA", "prod", &["ledgerA".to_string()]);

        // prod without a receipt -> MissingReceipt.
        let mut prod_no = frag_feed("issuerA", "prod", 1);
        prod_no.receipt = None;
        sign(&issuer_sk, &mut prod_no);
        assert_eq!(store.verify(&prod_no).unwrap_err(), FragmentError::MissingReceipt);

        // prod with a receipt from a different ledger -> ReceiptFromDisallowedLedger.
        let mut prod_wrong = frag_feed("issuerA", "prod", 1);
        signed_with_receipt(&issuer_sk, &mut prod_wrong, "ledgerZ", &ledger_a_sk);
        assert!(matches!(
            store.verify(&prod_wrong).unwrap_err(),
            FragmentError::ReceiptFromDisallowedLedger { .. }
        ));

        // prod with a valid receipt from the required ledger -> accepted.
        let mut prod_ok = frag_feed("issuerA", "prod", 1);
        signed_with_receipt(&issuer_sk, &mut prod_ok, "ledgerA", &ledger_a_sk);
        assert!(store.verify(&prod_ok).is_ok());

        // dev with no receipt -> accepted (no per-scope requirement, global off).
        let mut dev_no = frag_feed("issuerA", "dev", 1);
        dev_no.receipt = None;
        sign(&issuer_sk, &mut dev_no);
        assert!(store.verify(&dev_no).is_ok());
    }

    /// TC-F1.25 (FR-1f trust list): ledger key rotation — receipts signed by either the old
    /// or the new key of a ledger both verify.
    #[test]
    fn ledger_key_rotation() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (old_sk, old_pk) = keypair(20);
        let (new_sk, new_pk) = keypair(22);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        // Both keys are current for the same ledger (rotation window).
        store
            .load_transparency_trust_list(&[("ledgerA".into(), vec![old_pk, new_pk])])
            .unwrap();

        let mut f_old = frag_feed("issuerA", "", 1);
        signed_with_receipt(&issuer_sk, &mut f_old, "ledgerA", &old_sk);
        assert!(store.verify(&f_old).is_ok());

        let mut f_new = frag_feed("issuerA", "", 2);
        signed_with_receipt(&issuer_sk, &mut f_new, "ledgerA", &new_sk);
        assert!(store.verify(&f_new).is_ok());
    }

    /// TC-F1.26 (FR-1f back-compat): a legacy single-anchor configuration (mapped to the
    /// default ledger) behaves exactly as before — a valid anchor signature over the
    /// statement is accepted; a bogus receipt is rejected. Preserves TC-F1.15/16 semantics.
    #[test]
    fn legacy_single_anchor_backcompat() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (anchor_sk, anchor_pk) = keypair(9);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store.set_transparency_anchor(&anchor_pk).unwrap(); // -> default ledger

        // Legacy fragment: no receipt_ledger set (defaults to the default ledger).
        let mut f = frag_feed("issuerA", "", 1);
        f.signature = issuer_sk.sign(&f.signing_bytes()).to_bytes().to_vec();
        f.receipt = Some("deadbeef".into());
        f.receipt_ledger = None;
        assert_eq!(store.verify(&f).unwrap_err(), FragmentError::InvalidReceipt);

        let rsig = anchor_sk.sign(&f.signing_bytes());
        f.receipt = Some(rsig.to_bytes().iter().map(|b| format!("{:02x}", b)).collect());
        assert!(store.verify(&f).is_ok());
    }

    /// TC-F1.13 (FR-1e): a fragment for an undeclared (issuer, feed) is rejected.
    #[test]
    fn undeclared_feed_is_rejected() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap(); // declares default feed only
        let mut f = frag_feed("issuerA", "prod", 1);
        sign(&sk, &mut f);
        assert_eq!(
            store.load(&f).unwrap_err(),
            FragmentError::UndeclaredFeed {
                issuer: "issuerA".into(),
                feed: "prod".into()
            }
        );
        // After declaring the feed, the same fragment is accepted.
        store.declare_feed("issuerA", "prod", 0);
        assert!(store.load(&f).is_ok());
    }

    /// TC-F1.14 (FR-1e): the SVN floor is enforced per (issuer, feed) independently.
    #[test]
    fn svn_floor_is_per_feed() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();
        store.declare_feed("issuerA", "prod", 10);
        store.declare_feed("issuerA", "test", 0);

        // prod floor is 10: svn 5 rejected.
        let mut low = frag_feed("issuerA", "prod", 5);
        sign(&sk, &mut low);
        assert!(matches!(
            store.load(&low).unwrap_err(),
            FragmentError::RolledBackSvn { min_required: 10, .. }
        ));
        // test feed floor is 0: svn 1 accepted (independent of prod).
        let mut t = frag_feed("issuerA", "test", 1);
        sign(&sk, &mut t);
        assert!(store.load(&t).is_ok());
        // prod at floor accepted.
        let mut p = frag_feed("issuerA", "prod", 10);
        sign(&sk, &mut p);
        assert!(store.load(&p).is_ok());
    }

    /// TC-F1.15 / TC-F1.16 (FR-1f): with a transparency anchor configured, a fragment whose
    /// receipt is an invalid signature is rejected; a valid receipt (anchor signature over
    /// the statement) is accepted.
    #[test]
    fn transparency_receipt_is_cryptographically_verified() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (anchor_sk, anchor_pk) = keypair(9);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store.set_transparency_anchor(&anchor_pk).unwrap();

        // Build + issuer-sign a fragment.
        let mut f = frag_feed("issuerA", "", 1);
        f.signature = issuer_sk.sign(&f.signing_bytes()).to_bytes().to_vec();

        // Bogus receipt -> rejected.
        f.receipt = Some("deadbeef".into());
        assert_eq!(store.verify(&f).unwrap_err(), FragmentError::InvalidReceipt);

        // Valid receipt: anchor signs the same statement.
        let rsig = anchor_sk.sign(&f.signing_bytes());
        f.receipt = Some(rsig.to_bytes().iter().map(|b| format!("{:02x}", b)).collect());
        assert!(store.verify(&f).is_ok());
    }

    /// TC-F1.17 / TC-F1.18 / TC-F1.19 (FR-1g): a chain of fragments applies in dependency
    /// order; a fragment requiring an unloaded dependency is rejected; because `requires`
    /// can only reference already-loaded fragments, cycles/unbounded depth are impossible.
    #[test]
    fn fragment_chaining_requires_loaded_dependencies() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();

        // Base fragment (svn 1), id "issuerA//1".
        let mut base = frag_feed("issuerA", "", 1);
        sign(&sk, &mut base);
        let base_id = base.id();

        // Dependent fragment (svn 2) requires the base.
        let mut dep = frag_feed("issuerA", "", 2);
        dep.requires = vec![base_id.clone()];
        sign(&sk, &mut dep);

        // Loading the dependent before the base is rejected (broken link).
        assert_eq!(
            store.verify(&dep).unwrap_err(),
            FragmentError::UnsatisfiedRequirement {
                requires: base_id.clone()
            }
        );

        // Load the base first, then the dependent applies.
        store.load(&base).unwrap();
        assert!(store.load(&dep).is_ok());

        // A fragment requiring a never-loaded id is rejected wholesale.
        let mut orphan = frag_feed("issuerA", "", 3);
        orphan.requires = vec!["issuerA//999".into()];
        sign(&sk, &mut orphan);
        assert!(matches!(
            store.verify(&orphan).unwrap_err(),
            FragmentError::UnsatisfiedRequirement { .. }
        ));
    }

    /// TC-F1.21 (FR-1i): SVN high-water marks survive a restart via export/import, so a
    /// fragment at or below a previously-accepted SVN stays rejected after restart.
    #[test]
    fn svn_state_persists_across_restart() {
        let (sk, pk) = keypair(1);

        // First "boot": accept svn 5 on the default feed.
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();
        let mut f5 = frag_feed("issuerA", "", 5);
        sign(&sk, &mut f5);
        store.load(&f5).unwrap();
        let snapshot = store.export_svn_state();
        assert!(snapshot.contains("issuerA\t\t5"));

        // "Restart": a fresh store re-seeds issuers/floors, then imports the snapshot.
        let mut restarted = FragmentStore::new(true);
        restarted.authorize_issuer("issuerA", &pk).unwrap();
        restarted.import_svn_state(&snapshot);

        // A replay of svn 5 (or lower) is still rejected after restart.
        assert!(matches!(
            restarted.verify(&f5).unwrap_err(),
            FragmentError::RolledBackSvn { min_required: 6, .. }
        ));
        // svn 6 is accepted.
        let mut f6 = frag_feed("issuerA", "", 6);
        sign(&sk, &mut f6);
        assert!(restarted.load(&f6).is_ok());

        // import can only raise, never lower: importing an older snapshot is a no-op.
        restarted.import_svn_state("issuerA\t\t2");
        assert!(matches!(
            restarted.verify(&f6).unwrap_err(),
            FragmentError::RolledBackSvn { min_required: 7, .. }
        ));
    }

    /// TC-F1.20 (FR-1h): a fragment carried in a COSE_Sign1 envelope, signed by the
    /// authorized issuer over the fragment statement, verifies through the COSE path; a
    /// tampered envelope is rejected.
    #[test]
    fn cose_sign1_fragment_verifies() {
        use coset::{iana, CborSerializable, CoseSign1Builder, HeaderBuilder};

        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(true);
        store.authorize_issuer("issuerA", &pk).unwrap();

        // The fragment whose statement the COSE envelope carries.
        let fragment = PolicyFragment {
            issuer: "issuerA".into(),
            svn: 1,
            policy_module: Some("package agent_policy.fragments\nexec_allowed := true".into()),
            includes: vec!["exec".into()],
            receipt: Some("r1".into()),
            ..Default::default()
        };
        let statement = fragment.signing_bytes();

        // Build a COSE_Sign1 (EdDSA) with the statement as payload, signed by the issuer.
        let protected = HeaderBuilder::new().algorithm(iana::Algorithm::EdDSA).build();
        let sign1 = CoseSign1Builder::new()
            .protected(protected)
            .payload(statement.clone())
            .create_signature(b"", |tbs| sk.sign(tbs).to_bytes().to_vec())
            .build();
        let cose_bytes = sign1.to_vec().unwrap();

        // Verifies through the COSE path.
        let v = store.verify_cose(&fragment, &cose_bytes).unwrap();
        assert_eq!(v.issuer, "issuerA");
        assert!(v.policy_module.is_some());

        // A tampered envelope (flip a signature byte) is rejected.
        let mut tampered = cose_bytes.clone();
        let n = tampered.len();
        tampered[n - 1] ^= 0xff;
        assert!(store.verify_cose(&fragment, &tampered).is_err());

        // A COSE envelope whose payload does not match the presented fields is rejected.
        let mut other = fragment.clone();
        other.svn = 2;
        assert_eq!(
            store.verify_cose(&other, &cose_bytes).unwrap_err(),
            FragmentError::InvalidSignature
        );
    }
}
