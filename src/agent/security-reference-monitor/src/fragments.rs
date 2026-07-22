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
use sha2::{Digest, Sha256};
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
    /// FR-1j: the append-only log head this fragment asserts it is applied on top of. Bound
    /// into [`PolicyFragment::signing_bytes`] (the issuer signs it), so a host/orchestrator
    /// cannot forge an ordering. In ordered mode the store requires this to equal its
    /// current log head; on success the head advances by hashing this fragment in. `None` =
    /// not part of an ordered log (back-compat / opt-in).
    pub prev_log_head: Option<Vec<u8>>,
    /// FR-1f Stage 2: an optional transparency **inclusion + consistency** proof anchoring
    /// this fragment in an append-only transparency log (SCITT/CT). Text-encoded
    /// (`kata-ttl-proof/v1`): a signed tree head (size, root, ledger signature), the
    /// inclusion proof of this fragment's statement, and an optional consistency proof from
    /// the previously-seen tree head. Verified against the transparency trust list; proves
    /// the fragment is recorded in — and the log has only grown since the last — tree head.
    pub receipt_proof: Option<String>,
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
    /// sorted grants, module, sorted includes, sorted requires, and (FR-1j) the asserted
    /// predecessor log head — so none can be altered without invalidating the signature.
    /// The receipt is NOT included (it is a separate signature over these same bytes,
    /// created after the issuer signs); the `receipt_ledger` selector is also excluded.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut grants = self.grants.clone();
        grants.sort();
        let mut includes = self.includes.clone();
        includes.sort();
        let mut requires = self.requires.clone();
        requires.sort();
        let mut s = String::new();
        s.push_str("kata-policy-fragment/v3\n");
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
        // FR-1j: bind the asserted predecessor log head into the signature so ordering
        // cannot be forged by the (untrusted) delivery path. Empty when not part of an
        // ordered log.
        s.push_str("\n--prevhead--\n");
        if let Some(h) = &self.prev_log_head {
            for b in h {
                s.push_str(&format!("{:02x}", b));
            }
        }
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
    /// FR-1j: SHA-256 of the fragment statement, used to advance the ordering log head on
    /// commit (so the head binds the exact applied sequence).
    pub stmt_sha256: [u8; 32],
    /// FR-1f Stage 2: the verified transparency tree head `(ledger, size, root)` this
    /// fragment advanced to, applied (raise-only) on commit. `None` when no proof was given.
    pub ttl_head: Option<(String, u64, [u8; 32])>,
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
    /// FR-1d: the X.509 certificate chain (`x5chain`) is malformed, an unsupported
    /// algorithm, or a link signature does not verify.
    InvalidCertChain,
    /// FR-1d: no configured CA anchor is present in the presented certificate chain.
    UntrustedCa,
    /// FR-1d: the derived `did:x509` (chain CA + leaf policy) does not match an authorized
    /// anchor or the fragment's declared issuer.
    DidX509Mismatch,
    /// FR-1d: a certificate in the chain is on the measured revocation list.
    RevokedCertificate,
    /// FR-1d: a certificate in the chain is outside its validity window.
    CertExpired,
    /// FR-1j: the fragment asserts a predecessor log head that is not the store's current
    /// head — a reordering, omission, or insertion in the append-only application log.
    LogHeadMismatch { expected: String, presented: String },
    /// FR-1f Stage 2: the transparency inclusion proof does not recompute to the signed
    /// tree-head root (the fragment is not provably recorded in the log).
    InvalidInclusionProof,
    /// FR-1f Stage 2: the presented signed tree head is not an append-only extension of the
    /// last-seen one for this ledger (the log was rewound or forked).
    LogRolledBack { ledger: String, last_size: u64, presented_size: u64 },
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
            FragmentError::InvalidCertChain => write!(f, "invalid X.509 certificate chain"),
            FragmentError::UntrustedCa => write!(f, "no trusted CA anchor in certificate chain"),
            FragmentError::DidX509Mismatch => {
                write!(f, "did:x509 identity does not match an authorized anchor")
            }
            FragmentError::RevokedCertificate => write!(f, "certificate in chain is revoked"),
            FragmentError::CertExpired => write!(f, "certificate in chain is outside validity"),
            FragmentError::LogHeadMismatch { expected, presented } => write!(
                f,
                "fragment ordering log-head mismatch: expected {expected}, presented {presented}"
            ),
            FragmentError::InvalidInclusionProof => {
                write!(f, "transparency inclusion proof does not verify")
            }
            FragmentError::LogRolledBack { ledger, last_size, presented_size } => write!(
                f,
                "transparency log {ledger} rolled back: last size {last_size}, presented {presented_size}"
            ),
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
    /// FR-1d: authorized `did:x509` anchors (CA fingerprint + leaf policy), keyed by did.
    did_x509_anchors: HashMap<String, crate::did_x509::DidX509Anchor>,
    /// FR-1d: measured revocation list — SHA-256 fingerprints of revoked certificates.
    revoked_certs: HashSet<[u8; 32]>,
    /// FR-1d: when true, every fragment must present a valid `x5chain` (no raw-key path).
    require_x509: bool,
    /// FR-1j: the append-only application log head. Advances by hashing each committed
    /// fragment in. Equals the genesis until the first ordered fragment is committed.
    log_head: Vec<u8>,
    /// FR-1j: the log genesis; `Some` enables ordered mode (the log-head gate is enforced).
    log_genesis: Option<Vec<u8>>,
    /// FR-1j: the ordered, append-only record of committed fragments `(id, statement hash)`.
    ordered_log: Vec<(String, [u8; 32])>,
    /// FR-1j: number of fragments committed to the ordered log (persisted, raise-only, so a
    /// restart cannot rewind the log position).
    log_len: u64,
    /// FR-1f Stage 2: last-seen signed tree head `(size, root)` per transparency ledger.
    /// Monotonic (raise-only by size) and persisted, so the external log cannot be rewound
    /// across a restart.
    ttl_heads: HashMap<String, (u64, [u8; 32])>,
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

    /// FR-1d: authorize a `did:x509` issuer identity — a trusted CA (by fingerprint) plus a
    /// leaf policy. A fragment presenting an `x5chain` that path-validates to this CA and
    /// satisfies the policy is accepted as issued by `anchor.did`. Also declares the did's
    /// default feed so simple x509 fragments (no explicit feed) are accepted.
    pub fn authorize_did_x509(&mut self, anchor: crate::did_x509::DidX509Anchor) {
        self.feeds.entry((anchor.did.clone(), String::new())).or_insert(0);
        self.did_x509_anchors.insert(anchor.did.clone(), anchor);
    }

    /// FR-1d: set the measured certificate revocation list (SHA-256 fingerprints). Any chain
    /// containing a revoked certificate is rejected.
    pub fn set_revoked_certs(&mut self, fingerprints: impl IntoIterator<Item = [u8; 32]>) {
        self.revoked_certs = fingerprints.into_iter().collect();
    }

    /// FR-1d: require every fragment to present a valid `x5chain` (disables the raw-key
    /// path). Fail-closed: with this set, a fragment lacking an x509 chain is rejected.
    pub fn set_require_x509(&mut self, required: bool) {
        self.require_x509 = required;
    }

    /// Whether the raw-key issuer path is disabled (FR-1d strict x509 mode).
    pub fn require_x509(&self) -> bool {
        self.require_x509
    }

    /// Whether any `did:x509` anchor is configured.
    pub fn has_did_x509_anchors(&self) -> bool {
        !self.did_x509_anchors.is_empty()
    }

    /// FR-1j: enable append-only ordering by setting the log genesis (a measured constant).
    /// Idempotent for a fresh store; does not rewind an already-advanced head. When set, the
    /// log-head gate is enforced on every fragment.
    pub fn set_log_genesis(&mut self, genesis: &[u8]) {
        let g = genesis.to_vec();
        if self.log_genesis.is_none() && self.log_len == 0 {
            self.log_head = g.clone();
        }
        self.log_genesis = Some(g);
    }

    /// FR-1j: whether ordered mode is enabled.
    pub fn is_ordered(&self) -> bool {
        self.log_genesis.is_some()
    }

    /// FR-1j: the current append-only log head (the value the next fragment must assert as
    /// its `prev_log_head`).
    pub fn log_head(&self) -> &[u8] {
        &self.log_head
    }

    /// FR-1j: export the ordered application log as a deterministic, customer-auditable
    /// record: one `index\tfragment-id\tstatement-sha256` line per committed fragment, then
    /// a final `head\t<hex>` line. This is the non-repudiable proof of the exact applied
    /// sequence (empty when not in ordered mode / nothing committed this session).
    pub fn export_fragment_log(&self) -> String {
        let mut out = String::new();
        for (i, (id, hash)) in self.ordered_log.iter().enumerate() {
            out.push_str(&format!("{}\t{}\t{}\n", i, id, bytes_to_hex(hash)));
        }
        out.push_str(&format!("head\t{}", bytes_to_hex(&self.log_head)));
        out
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

    /// FR-1d: verify a fragment whose COSE_Sign1 envelope carries a `did:x509` certificate
    /// chain (`x5chain`). The chain is path-validated to a trusted CA anchor, the leaf must
    /// satisfy the anchor's `did:x509` policy, no chain certificate may be revoked, and the
    /// leaf key must sign the fragment statement. The derived `did` must equal the
    /// fragment's declared issuer. After identity verification the same gates as
    /// [`verify`](Self::verify) apply (feed, SVN, receipt, requires, add-only).
    pub fn verify_cose_x509(
        &self,
        fragment: &PolicyFragment,
        cose_sign1: &[u8],
    ) -> Result<VerifiedFragment, FragmentError> {
        let statement = fragment.signing_bytes();
        let did = crate::did_x509::verify_x509_cose(
            &self.did_x509_anchors,
            &self.revoked_certs,
            cose_sign1,
            &statement,
        )?;
        if did != fragment.issuer {
            return Err(FragmentError::DidX509Mismatch);
        }
        self.check_gates(fragment, &statement)
    }
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

        // 5. Transparency receipt (FR-1f): a receipt may be required globally or per-scope
        //    (`required_receipt_from`). Two forms are accepted and both are scoped by
        //    `allowed_ledgers`/`required_receipt_from`:
        //      Stage 1 — a detached signature by a trust-list ledger key over the statement;
        //      Stage 2 — an inclusion + consistency proof anchoring the statement in an
        //                append-only transparency log at a signed, monotonic tree head.
        let receipt = fragment.receipt.as_deref().unwrap_or("");
        let proof = fragment.receipt_proof.as_deref().unwrap_or("");
        let ledger = fragment
            .receipt_ledger
            .as_deref()
            .filter(|l| !l.is_empty())
            .unwrap_or(DEFAULT_LEDGER);
        let has_receipt = !receipt.is_empty() || !proof.is_empty();

        // Per-scope required ledgers (policy-driven `required_receipts`). A non-empty list
        // makes a receipt mandatory for this scope and constrains its ledger.
        let required = self.required_receipt_from.get(&feed_key);
        let scope_requires = required.map(|r| !r.is_empty()).unwrap_or(false);

        let mut ttl_head: Option<(String, u64, [u8; 32])> = None;

        if !has_receipt {
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
            let keys = self
                .transparency_trust_list
                .get(ledger)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            // Stage 1: detached-signature receipt, verified against any current ledger key
            // (rotation) when a trust list is configured.
            if !receipt.is_empty() && !self.transparency_trust_list.is_empty() {
                let bytes = hex_to_bytes(receipt).map_err(|_| FragmentError::InvalidReceipt)?;
                let rsig = Signature::from_slice(&bytes).map_err(|_| FragmentError::InvalidReceipt)?;
                if !keys.iter().any(|k| k.verify(statement, &rsig).is_ok()) {
                    return Err(FragmentError::InvalidReceipt);
                }
            }

            // Stage 2: transparency inclusion + consistency proof.
            if !proof.is_empty() {
                let tp = TransparencyProof::parse(proof).ok_or(FragmentError::InvalidInclusionProof)?;
                // (a) the signed tree head must be signed by a current ledger key.
                let sth = sth_signing_bytes(ledger, tp.size, &tp.root);
                let ssig = Signature::from_slice(&tp.sig).map_err(|_| FragmentError::InvalidReceipt)?;
                if !keys.iter().any(|k| k.verify(&sth, &ssig).is_ok()) {
                    return Err(FragmentError::InvalidReceipt);
                }
                // (b) the statement must be included in the tree at that head.
                let leaf = crate::merkle::leaf_hash(statement);
                if !crate::merkle::verify_inclusion(tp.index, tp.size, leaf, &tp.incl, &tp.root) {
                    return Err(FragmentError::InvalidInclusionProof);
                }
                // (c) the head must be an append-only extension of the last-seen head
                //     (monotonic + consistency-proven) — this is the ordering guarantee.
                if let Some((last_size, last_root)) = self.ttl_heads.get(ledger) {
                    if tp.size < *last_size {
                        return Err(FragmentError::LogRolledBack {
                            ledger: ledger.to_string(),
                            last_size: *last_size,
                            presented_size: tp.size,
                        });
                    } else if tp.size == *last_size {
                        if &tp.root != last_root {
                            return Err(FragmentError::LogRolledBack {
                                ledger: ledger.to_string(),
                                last_size: *last_size,
                                presented_size: tp.size,
                            });
                        }
                    } else if !crate::merkle::verify_consistency(
                        *last_size, tp.size, last_root, &tp.root, &tp.cons,
                    ) {
                        return Err(FragmentError::LogRolledBack {
                            ledger: ledger.to_string(),
                            last_size: *last_size,
                            presented_size: tp.size,
                        });
                    }
                }
                ttl_head = Some((ledger.to_string(), tp.size, tp.root));
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

        // 8. FR-1j: append-only ordering. In ordered mode the fragment's signed
        //    `prev_log_head` must equal the store's current head, so any reordering,
        //    omission, or insertion (which would present a different predecessor head) is
        //    rejected fail-closed. `prev_log_head` is bound into `signing_bytes`, so the
        //    untrusted delivery path cannot forge it.
        let stmt_sha256: [u8; 32] = Sha256::digest(statement).into();
        if self.log_genesis.is_some() {
            let presented = fragment.prev_log_head.as_deref().unwrap_or(&[]);
            if presented != self.log_head.as_slice() {
                return Err(FragmentError::LogHeadMismatch {
                    expected: bytes_to_hex(&self.log_head),
                    presented: bytes_to_hex(presented),
                });
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
            stmt_sha256,
            ttl_head,
        })
    }

    /// Commit a previously [`verify`](Self::verify)-ed fragment: advance the `(issuer, feed)`
    /// SVN high-water mark, record the fragment id (for composition), and accumulate its
    /// grants. Returns the grants newly added.
    pub fn commit(&mut self, verified: &VerifiedFragment) -> Vec<String> {
        self.last_svn
            .insert((verified.issuer.clone(), verified.feed.clone()), verified.svn);
        self.loaded_ids.insert(verified.id.clone());
        // FR-1j: advance the append-only ordering log head (ordered mode only).
        if self.log_genesis.is_some() {
            let mut h = Sha256::new();
            h.update(&self.log_head);
            h.update(verified.stmt_sha256);
            self.log_head = h.finalize().to_vec();
            self.ordered_log
                .push((verified.id.clone(), verified.stmt_sha256));
            self.log_len += 1;
        }
        // FR-1f Stage 2: advance the per-ledger transparency tree head (raise-only by size).
        if let Some((ledger, size, root)) = &verified.ttl_head {
            let entry = self.ttl_heads.entry(ledger.clone()).or_insert((0, [0u8; 32]));
            if *size >= entry.0 {
                *entry = (*size, *root);
            }
        }
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
        // FR-1j: persist the ordering log head + length (raise-only on import) so a restart
        // cannot rewind the append-only log position. Uses a reserved sentinel key that can
        // never collide with an issuer id (issuers cannot contain a tab).
        if self.log_genesis.is_some() {
            lines.push(format!(
                "{LOG_STATE_KEY}\t{}\t{}",
                bytes_to_hex(&self.log_head),
                self.log_len
            ));
        }
        // FR-1f Stage 2: persist each ledger's last-seen transparency tree head (raise-only).
        let mut ttl: Vec<String> = self
            .ttl_heads
            .iter()
            .map(|(ledger, (size, root))| {
                format!("{TTL_STATE_KEY}\t{ledger}\t{size}\t{}", bytes_to_hex(root))
            })
            .collect();
        ttl.sort();
        lines.extend(ttl);
        lines.join("\n")
    }

    /// FR-1i: import a persisted SVN snapshot on boot. Each entry can only **raise** the
    /// high-water mark for its `(issuer, feed)`, never lower it — so an agent/VM restart can
    /// never reopen a rollback window (a fragment at or below a previously-accepted SVN
    /// stays rejected). Combined with the declarative floor (FR-1e), the effective minimum
    /// is `max(declared floor, persisted high-water + 1)`. FR-1j restores the ordering log
    /// head and FR-1f Stage 2 the per-ledger transparency tree head, both raise-only, so
    /// neither can be rewound across a restart.
    pub fn import_svn_state(&mut self, snapshot: &str) {
        for line in snapshot.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            match parts.as_slice() {
                [k, head, len] if *k == LOG_STATE_KEY => {
                    // FR-1j: ordering log head + length. Raise-only.
                    if let (Ok(head), Ok(len)) = (hex_to_bytes(head), len.trim().parse::<u64>()) {
                        if len >= self.log_len {
                            self.log_head = head;
                            self.log_len = len;
                        }
                    }
                }
                [k, ledger, size, root] if *k == TTL_STATE_KEY => {
                    // FR-1f Stage 2: transparency tree head. Raise-only by size.
                    if let (Ok(size), Ok(root)) = (size.trim().parse::<u64>(), hex_to_bytes(root)) {
                        if root.len() == 32 {
                            let mut r = [0u8; 32];
                            r.copy_from_slice(&root);
                            let entry = self.ttl_heads.entry(ledger.to_string()).or_insert((0, [0u8; 32]));
                            if size >= entry.0 {
                                *entry = (size, r);
                            }
                        }
                    }
                }
                [issuer, feed, svn] => {
                    if let Ok(svn) = svn.trim().parse::<u64>() {
                        let entry = self
                            .last_svn
                            .entry((issuer.to_string(), feed.to_string()))
                            .or_insert(0);
                        *entry = (*entry).max(svn);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Reserved sentinel key for the FR-1j ordering-log state line in the SVN snapshot.
const LOG_STATE_KEY: &str = "--log-state--";
/// Reserved sentinel key for the FR-1f Stage 2 per-ledger tree-head state line.
const TTL_STATE_KEY: &str = "--ttl-head--";

/// FR-1f Stage 2: the bytes a transparency ledger signs to attest a tree head — binds the
/// ledger id, tree size, and Merkle root so a signed head cannot be replayed for a different
/// ledger or size. Public so ledger tooling/tests produce byte-identical signed heads.
pub fn sth_signing_bytes(ledger: &str, size: u64, root: &[u8; 32]) -> Vec<u8> {
    format!("kata-sth/v1\n{ledger}\n{size}\n{}", bytes_to_hex(root)).into_bytes()
}

/// FR-1f Stage 2: encode a `kata-ttl-proof/v1` transparency proof exactly as the verifier
/// parses it (signed tree head + inclusion proof + optional consistency proof). Used by the
/// mock-ledger dev tool, the demo, and tests so the wire format has a single source of truth.
pub fn encode_transparency_proof(
    size: u64,
    root: &[u8; 32],
    sig: &[u8],
    index: u64,
    incl: &[[u8; 32]],
    cons: &[[u8; 32]],
) -> String {
    let join = |v: &[[u8; 32]]| v.iter().map(bytes_to_hex_arr).collect::<Vec<_>>().join(",");
    format!(
        "kata-ttl-proof/v1\nsize={}\nroot={}\nsig={}\nindex={}\nincl={}\ncons={}\n",
        size,
        bytes_to_hex(root),
        bytes_to_hex(sig),
        index,
        join(incl),
        join(cons)
    )
}

fn bytes_to_hex_arr(b: &[u8; 32]) -> String {
    bytes_to_hex(b)
}

/// FR-1f Stage 2: a parsed `kata-ttl-proof/v1` transparency proof (signed tree head +
/// inclusion proof + optional consistency proof).
struct TransparencyProof {
    size: u64,
    root: [u8; 32],
    sig: Vec<u8>,
    index: u64,
    incl: Vec<[u8; 32]>,
    cons: Vec<[u8; 32]>,
}

impl TransparencyProof {
    fn parse(s: &str) -> Option<Self> {
        let mut size = None;
        let mut root = None;
        let mut sig = None;
        let mut index = None;
        let mut incl = Vec::new();
        let mut cons = Vec::new();
        let mut header_ok = false;
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "kata-ttl-proof/v1" {
                header_ok = true;
                continue;
            }
            let (k, v) = line.split_once('=')?;
            match k {
                "size" => size = v.trim().parse::<u64>().ok(),
                "root" => root = parse_hash32(v),
                "sig" => sig = hex_to_bytes(v).ok(),
                "index" => index = v.trim().parse::<u64>().ok(),
                "incl" => incl = parse_hash_list(v)?,
                "cons" => cons = parse_hash_list(v)?,
                _ => {}
            }
        }
        if !header_ok {
            return None;
        }
        Some(TransparencyProof {
            size: size?,
            root: root?,
            sig: sig?,
            index: index?,
            incl,
            cons,
        })
    }
}

fn parse_hash32(v: &str) -> Option<[u8; 32]> {
    let b = hex_to_bytes(v).ok()?;
    if b.len() != 32 {
        return None;
    }
    let mut h = [0u8; 32];
    h.copy_from_slice(&b);
    Some(h)
}

fn parse_hash_list(v: &str) -> Option<Vec<[u8; 32]>> {
    let v = v.trim();
    if v.is_empty() {
        return Some(Vec::new());
    }
    v.split(',').map(|e| parse_hash32(e.trim())).collect()
}

/// Lower-case hex encoding.
fn bytes_to_hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{:02x}", byte));
    }
    s
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
    fn ordered_frag(issuer: &str, svn: u64, prev_head: &[u8]) -> PolicyFragment {
        PolicyFragment {
            issuer: issuer.to_string(),
            svn,
            prev_log_head: Some(prev_head.to_vec()),
            ..Default::default()
        }
    }

    /// TC-F1.28 (FR-1j): fragments applied in order are accepted and the append-only log
    /// head advances deterministically; the exported log records the exact sequence.
    #[test]
    fn ordering_in_order_accepted_and_head_advances() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &pk).unwrap();
        store.set_log_genesis(b"kata-fragment-log/test-genesis");

        let h0 = store.log_head().to_vec();
        let mut a = ordered_frag("issuerA", 1, &h0);
        sign(&sk, &mut a);
        assert!(store.load(&a).is_ok());
        let h1 = store.log_head().to_vec();
        assert_ne!(h0, h1, "head must advance");

        let mut b = ordered_frag("issuerA", 2, &h1);
        sign(&sk, &mut b);
        assert!(store.load(&b).is_ok());
        let h2 = store.log_head().to_vec();
        assert_ne!(h1, h2);

        // The exported log is deterministic and ends with the current head.
        let log = store.export_fragment_log();
        assert!(log.contains("0\tissuerA//1\t"));
        assert!(log.contains("1\tissuerA//2\t"));
        assert!(log.trim_end().ends_with(&super::bytes_to_hex(&h2)));
    }

    /// TC-F1.29 (FR-1j): a fragment asserting a stale/wrong predecessor head (a reordering,
    /// omission, or insertion) is rejected fail-closed and the store is unchanged.
    #[test]
    fn ordering_out_of_order_rejected() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &pk).unwrap();
        store.set_log_genesis(b"kata-fragment-log/test-genesis");

        let h0 = store.log_head().to_vec();
        let mut a = ordered_frag("issuerA", 1, &h0);
        sign(&sk, &mut a);
        store.load(&a).unwrap();
        let h1 = store.log_head().to_vec();

        // A second fragment that still asserts the genesis head (out of order) is rejected.
        let mut stale = ordered_frag("issuerA", 2, &h0);
        sign(&sk, &mut stale);
        assert!(matches!(
            store.load(&stale).unwrap_err(),
            FragmentError::LogHeadMismatch { .. }
        ));
        // Head unchanged after the rejected fragment (fail-closed).
        assert_eq!(store.log_head(), h1.as_slice());

        // The correct next fragment (asserting h1) is accepted.
        let mut b = ordered_frag("issuerA", 2, &h1);
        sign(&sk, &mut b);
        assert!(store.load(&b).is_ok());
    }

    /// TC-F1.30 (FR-1j): the ordering log head survives export/import (restart) and is
    /// raise-only — after a restart the next in-order fragment is accepted, a stale one is
    /// rejected, and importing an older (shorter) snapshot cannot rewind the head.
    #[test]
    fn ordering_head_persists_across_restart_raise_only() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &pk).unwrap();
        store.set_log_genesis(b"kata-fragment-log/test-genesis");

        let h0 = store.log_head().to_vec();
        let mut a = ordered_frag("issuerA", 1, &h0);
        sign(&sk, &mut a);
        store.load(&a).unwrap();
        let snap_after_a = store.export_svn_state();
        let h1 = store.log_head().to_vec();

        let mut b = ordered_frag("issuerA", 2, &h1);
        sign(&sk, &mut b);
        store.load(&b).unwrap();
        let snap_after_b = store.export_svn_state();
        let h2 = store.log_head().to_vec();

        // Restart: fresh store, re-seed genesis, import the persisted state.
        let mut restarted = FragmentStore::new(false);
        restarted.authorize_issuer("issuerA", &pk).unwrap();
        restarted.set_log_genesis(b"kata-fragment-log/test-genesis");
        restarted.import_svn_state(&snap_after_b);
        assert_eq!(restarted.log_head(), h2.as_slice(), "head restored across restart");

        // The next in-order fragment (prev = h2) is accepted after restart.
        let mut c = ordered_frag("issuerA", 3, &h2);
        sign(&sk, &mut c);
        assert!(restarted.load(&c).is_ok());

        // Raise-only: importing the older (shorter) snapshot must NOT rewind the head.
        let head_now = restarted.log_head().to_vec();
        restarted.import_svn_state(&snap_after_a);
        assert_eq!(restarted.log_head(), head_now.as_slice(), "older snapshot cannot rewind");
    }

    /// TC-F1.30b (FR-1j): non-ordered mode is unchanged — with no genesis configured, a
    /// fragment carrying no prev_log_head is accepted (opt-in / back-compat).
    #[test]
    fn ordering_disabled_by_default() {
        let (sk, pk) = keypair(1);
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &pk).unwrap();
        assert!(!store.is_ordered());
        let mut f = frag("issuerA", 1, &["exec:x"]);
        f.receipt = None;
        sign(&sk, &mut f);
        assert!(store.load(&f).is_ok());
    }
    // ---- FR-1f Stage 2: transparency inclusion + consistency proofs ----
    use crate::merkle::MerkleTree;

    fn ttl_proof(tree: &MerkleTree, sk: &SigningKey, ledger: &str, index: usize, cons_from: Option<usize>) -> String {
        let size = tree.size();
        let root = tree.root();
        let sig = sk.sign(&sth_signing_bytes(ledger, size, &root)).to_bytes();
        let incl = tree.inclusion_proof(index);
        let cons = cons_from.map(|m| tree.consistency_proof(m)).unwrap_or_default();
        encode_transparency_proof(size, &root, &sig, index as u64, &incl, &cons)
    }

    fn ttl_frag(issuer_sk: &SigningKey, svn: u64, ledger: &str) -> PolicyFragment {
        let mut f = PolicyFragment { issuer: "issuerA".into(), svn, receipt_ledger: Some(ledger.into()), ..Default::default() };
        f.signature = issuer_sk.sign(&f.signing_bytes()).to_bytes().to_vec();
        f
    }

    /// TC-F1.32 (Stage 2): a valid inclusion proof under a signed tree head is accepted; a
    /// tampered inclusion proof (wrong leaf position) is rejected.
    #[test]
    fn stage2_inclusion_proof_verified() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (led_sk, led_pk) = keypair(30);
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store.load_transparency_trust_list(&[("ttl".into(), vec![led_pk])]).unwrap();

        let f = ttl_frag(&issuer_sk, 1, "ttl");
        let mut tree = MerkleTree::new();
        tree.push(f.signing_bytes());
        let mut ok = f.clone();
        ok.receipt_proof = Some(ttl_proof(&tree, &led_sk, "ttl", 0, None));
        assert!(store.verify(&ok).is_ok());

        // Wrong index (leaf not at claimed position) -> inclusion fails.
        let mut bad = f.clone();
        tree.push(b"other".to_vec());
        bad.receipt_proof = Some(ttl_proof(&tree, &led_sk, "ttl", 1, None)); // claims index 1, but leaf 1 is "other"
        assert_eq!(store.verify(&bad).unwrap_err(), FragmentError::InvalidInclusionProof);
    }

    /// TC-F1.33 (Stage 2): a signed tree head signed by a key NOT in the trust list is
    /// rejected (the ledger signature must chain to a trusted key).
    #[test]
    fn stage2_untrusted_sth_rejected() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (_led_sk, led_pk) = keypair(30);
        let (evil_sk, _evil_pk) = keypair(31);
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store.load_transparency_trust_list(&[("ttl".into(), vec![led_pk])]).unwrap();

        let f = ttl_frag(&issuer_sk, 1, "ttl");
        let mut tree = MerkleTree::new();
        tree.push(f.signing_bytes());
        let mut bad = f.clone();
        bad.receipt_proof = Some(ttl_proof(&tree, &evil_sk, "ttl", 0, None)); // signed by untrusted key
        assert_eq!(store.verify(&bad).unwrap_err(), FragmentError::InvalidReceipt);
    }

    /// TC-F1.34 (Stage 2): the tree head is monotonic — a growing log with a valid
    /// consistency proof is accepted; a shrunk head (rollback) is rejected.
    #[test]
    fn stage2_consistency_and_rollback() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (led_sk, led_pk) = keypair(30);
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store.load_transparency_trust_list(&[("ttl".into(), vec![led_pk])]).unwrap();

        let fa = ttl_frag(&issuer_sk, 1, "ttl");
        let fb = ttl_frag(&issuer_sk, 2, "ttl");

        // Log state after A: size 1.
        let mut t1 = MerkleTree::new();
        t1.push(fa.signing_bytes());
        let mut a = fa.clone();
        a.receipt_proof = Some(ttl_proof(&t1, &led_sk, "ttl", 0, None));
        assert!(store.load(&a).is_ok());

        // Log state after B: size 2, with a consistency proof from size 1.
        let mut t2 = MerkleTree::new();
        t2.push(fa.signing_bytes());
        t2.push(fb.signing_bytes());
        let mut b = fb.clone();
        b.receipt_proof = Some(ttl_proof(&t2, &led_sk, "ttl", 1, Some(1)));
        assert!(store.load(&b).is_ok());

        // A fragment presenting an OLDER (size 1) head after the head advanced to 2 -> rollback.
        let fc = ttl_frag(&issuer_sk, 3, "ttl");
        let mut t1b = MerkleTree::new();
        t1b.push(fc.signing_bytes());
        let mut c = fc.clone();
        c.receipt_proof = Some(ttl_proof(&t1b, &led_sk, "ttl", 0, None));
        assert!(matches!(store.verify(&c).unwrap_err(), FragmentError::LogRolledBack { .. }));
    }

    /// TC-F1.35 (Stage 2): the transparency tree head survives export/import (restart) and
    /// is raise-only — after restart an older head is still rejected.
    #[test]
    fn stage2_tree_head_persists_across_restart() {
        let (issuer_sk, issuer_pk) = keypair(1);
        let (led_sk, led_pk) = keypair(30);
        let mut store = FragmentStore::new(false);
        store.authorize_issuer("issuerA", &issuer_pk).unwrap();
        store.load_transparency_trust_list(&[("ttl".into(), vec![led_pk])]).unwrap();

        let fa = ttl_frag(&issuer_sk, 1, "ttl");
        let fb = ttl_frag(&issuer_sk, 2, "ttl");
        let mut t2 = MerkleTree::new();
        t2.push(fa.signing_bytes());
        t2.push(fb.signing_bytes());
        // Jump straight to head size 2 (A already logged elsewhere): load B at size 2.
        let mut a = fa.clone();
        let mut t1 = MerkleTree::new();
        t1.push(fa.signing_bytes());
        a.receipt_proof = Some(ttl_proof(&t1, &led_sk, "ttl", 0, None));
        store.load(&a).unwrap();
        let mut b = fb.clone();
        b.receipt_proof = Some(ttl_proof(&t2, &led_sk, "ttl", 1, Some(1)));
        store.load(&b).unwrap();
        let snap = store.export_svn_state();
        assert!(snap.contains("--ttl-head--\tttl\t2\t"));

        // Restart: fresh store, same trust list, import the persisted tree head.
        let mut restarted = FragmentStore::new(false);
        restarted.authorize_issuer("issuerA", &issuer_pk).unwrap();
        restarted.load_transparency_trust_list(&[("ttl".into(), vec![led_pk])]).unwrap();
        restarted.import_svn_state(&snap);

        // An older (size 1) head is rejected after restart (raise-only tree head).
        let fc = ttl_frag(&issuer_sk, 3, "ttl");
        let mut t1c = MerkleTree::new();
        t1c.push(fc.signing_bytes());
        let mut c = fc.clone();
        c.receipt_proof = Some(ttl_proof(&t1c, &led_sk, "ttl", 0, None));
        assert!(matches!(restarted.verify(&c).unwrap_err(), FragmentError::LogRolledBack { .. }));
    }
}

