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

/// A policy fragment presented for loading.
#[derive(Debug, Clone, Default)]
pub struct PolicyFragment {
    /// Identifier of the issuer that signed the fragment.
    pub issuer: String,
    /// Security version number; must strictly increase per issuer.
    pub svn: u64,
    /// Additional grants introduced by this fragment (add-only). Legacy/opaque form.
    pub grants: Vec<String>,
    /// FR-1c: a signed Rego module (text) the fragment contributes to the policy engine
    /// (Model A). Must declare a package under the reserved fragment namespace.
    pub policy_module: Option<String>,
    /// FR-1c: the policy namespaces this fragment is scoped to contribute to. The applier
    /// refuses a module whose package is outside these.
    pub includes: Vec<String>,
    /// Transparency receipt identifier/blob. Required when receipts are enforced.
    pub receipt: Option<String>,
    /// Detached Ed25519 signature over [`PolicyFragment::signing_bytes`].
    pub signature: Vec<u8>,
}

impl PolicyFragment {
    /// Canonical byte encoding that the signature covers. Deterministic: the issuer, the
    /// SVN, the sorted grants, the contributed Rego module, the sorted `includes`
    /// namespaces, and the receipt are all bound, so none can be altered without
    /// invalidating the signature.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut grants = self.grants.clone();
        grants.sort();
        let mut includes = self.includes.clone();
        includes.sort();
        let mut s = String::new();
        s.push_str("kata-policy-fragment/v1\n");
        s.push_str(&self.issuer);
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
        s.push_str("--module--\n");
        s.push_str(self.policy_module.as_deref().unwrap_or(""));
        s.push('\n');
        s.push_str("--receipt--\n");
        s.push_str(self.receipt.as_deref().unwrap_or(""));
        s.into_bytes()
    }
}

/// A fragment that has passed every verification gate but has not yet been committed to the
/// store. Returned by [`FragmentStore::verify`] so the caller can apply it to the policy
/// engine and only then [`FragmentStore::commit`] it — keeping verify+apply atomic.
#[derive(Debug, Clone)]
pub struct VerifiedFragment {
    pub issuer: String,
    pub svn: u64,
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
    last_svn: HashMap<String, u64>,
    root_constraints: HashSet<String>,
    require_receipt: bool,
    active_grants: HashSet<String>,
    /// FR-1b: declarative per-issuer SVN floor seeded from measured state.
    min_svn: HashMap<String, u64>,
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
        self.issuers.insert(issuer.into(), key);
        Ok(())
    }

    /// FR-1b: set a declarative minimum-SVN floor for an issuer (from measured state). A
    /// fragment from this issuer is only accepted at `svn >= floor`.
    pub fn set_min_svn(&mut self, issuer: impl Into<String>, floor: u64) {
        self.min_svn.insert(issuer.into(), floor);
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

    /// The minimum SVN an issuer's next fragment must carry (declarative floor combined
    /// with the monotonic high-water mark of accepted fragments).
    fn min_required(&self, issuer: &str) -> u64 {
        let floor = self.min_svn.get(issuer).copied().unwrap_or(0);
        match self.last_svn.get(issuer) {
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

        // 2. Signature must verify over the canonical bytes (rejects unsigned/tampered).
        let sig =
            Signature::from_slice(&fragment.signature).map_err(|_| FragmentError::InvalidSignature)?;
        key.verify(&fragment.signing_bytes(), &sig)
            .map_err(|_| FragmentError::InvalidSignature)?;

        // 3. Monotonic SVN: >= the declarative floor and strictly greater than the last
        //    accepted for this issuer.
        let min_required = self.min_required(&fragment.issuer);
        if fragment.svn < min_required {
            return Err(FragmentError::RolledBackSvn {
                issuer: fragment.issuer.clone(),
                presented: fragment.svn,
                min_required,
            });
        }

        // 4. Transparency receipt required in strict mode.
        if self.require_receipt && fragment.receipt.as_deref().unwrap_or("").is_empty() {
            return Err(FragmentError::MissingReceipt);
        }

        // 5. Add-only: reject any grant that would relax a root constraint.
        for g in &fragment.grants {
            if self.root_constraints.contains(g) {
                return Err(FragmentError::RootConstraintRelaxation(g.clone()));
            }
        }

        Ok(VerifiedFragment {
            issuer: fragment.issuer.clone(),
            svn: fragment.svn,
            grants: fragment.grants.clone(),
            policy_module: fragment.policy_module.clone(),
            includes: fragment.includes.clone(),
        })
    }

    /// Commit a previously [`verify`](Self::verify)-ed fragment: advance the issuer's SVN
    /// high-water mark and accumulate its grants. Returns the grants newly added.
    pub fn commit(&mut self, verified: &VerifiedFragment) -> Vec<String> {
        self.last_svn.insert(verified.issuer.clone(), verified.svn);
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
}
