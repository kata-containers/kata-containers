# FR-1 — Signed, add-only policy fragments (detailed reference)

**What this is.** The full technical breakdown of the signed-policy-fragments feature, the
largest of the PARMA execution-integrity hardening features. The main guide
(`parma-hardening-features.md`) carries a summary; this document is the authoritative,
sub-requirement-by-sub-requirement reference with code locations, error variants, tests, and
the per-commit map. Reproducible dev walk-through: `docs/cc/fr1-fragment-e2e.md`.

All code lives in `src/agent/security-reference-monitor/src/` (the SRM crate) with agent
wiring in `src/agent/src/{main.rs,rpc.rs}` and tooling in
`src/agent/security-reference-monitor/examples/` + `src/tools/agent-ctl/`.

---

## 1. The guarantee

A base policy is one-shot/immutable within an attestation epoch (FR-12). Fragments let a
deployment **extend** what a workload may do at runtime **without weakening that guarantee**:
every extension is authenticated, monotonic, scope-limited, ordered, and incapable of relaxing
a base invariant. Concretely, only a signed, non-rolled-back, in-order, scope-limited policy
extension from an attested-trusted issuer can change what the workload may do — and every such
change is auditable after the fact.

**Threat model.** The host/orchestrator delivering fragments over the guest RPC is untrusted.
It may reorder, replay, omit, tamper, or forge fragments and their metadata. Every gate below
is fail-closed: on any doubt the fragment is rejected and neither the policy engine nor the
fragment store is mutated.

---

## 2. Sub-requirement map

| Sub-req | Guarantee it adds | Core code | Commits |
|---|---|---|---|
| **FR-1a** | verified fragment module is applied to the **live** policy engine (additive `add_policy`, never touches the one-shot `set_policy` lock) | `policy.rs::apply_fragment_module`; `rpc.rs::load_policy_fragment` | `dd2630053`, `294353a2a`, `11285337c`, `4ccd43f8a` |
| **FR-1b** | **authorized issuer** (measured trust root) + declarative **min-SVN floor**; atomic verify→apply→commit | `fragments.rs::{authorize_issuer,set_min_svn,verify,commit}`; `main.rs::seed_fragment_trust_root` | `bf602cb18`, `294353a2a` |
| **FR-1c** | **structured payload** — signed Rego module + `includes` namespace scoping | `fragments.rs` (`policy_module`, `includes`); applier namespace check | `bf602cb18` |
| **FR-1d** | **did:x509 issuer identity** — X.509 chain in COSE `x5chain`, CA-fingerprint + policy anchor, revocation, rotation, `basicConstraints CA:TRUE` on issuers; multi-algorithm (ES256/ES384/RS256/PS256) leaf + chain | `did_x509.rs`, `cose_keys.rs::{verify_cose,verify_cert_sig}` | `9cddd7f75`, `d6cdba49e`, `d01fabe13`, `bl2` |
| **FR-1e** | **feed scoping** — accepted `(issuer, feed)` pairs declared; undeclared feed rejected; per-feed SVN floor | `fragments.rs::declare_feed`; `UndeclaredFeed` | `ff8a4d5b9`, `c6b52c2ba` |
| **FR-1f** | **transparency receipts + Trust List** — multi-ledger, `allowed_ledgers` scoping, policy-driven `required_receipts`, ledger key rotation | `fragments.rs::{load_transparency_trust_list,set_allowed_ledgers,require_receipt_for}` | `ff8a4d5b9`, `c6b52c2ba`, `db24d40f5` |
| **FR-1f Stage 2** | **transparency-log inclusion + consistency** — RFC 6962 Merkle inclusion proof + monotonic, persisted signed tree head (append-only log) | `merkle.rs`; `fragments.rs` (`TransparencyProof`, `ttl_heads`) | `62fb8d45a` |
| **FR-1g** | **composition** — a fragment may `require` already-loaded fragments (no cycles/unbounded depth) | `fragments.rs` (`requires`); `UnsatisfiedRequirement` | `ff8a4d5b9`, `c6b52c2ba` |
| **FR-1h** | **COSE_Sign1 envelope** interop (pure-Rust `coset`, no Go) | `fragments.rs::verify_cose` | `c0ea3cb25`, `f7ed23319`, `93e1ff6e5` |
| **FR-1i** | **SVN rollback protection across restart** — raise-only persisted high-water marks | `fragments.rs::{export_svn_state,import_svn_state}`; `main.rs` persist | `c0ea3cb25`, `f7ed23319` |
| **FR-1j** | **append-only application ordering** — signed rolling log head; reject reorder/omit/insert; exportable auditable log | `fragments.rs::{set_log_genesis,log_head,export_fragment_log}` (gate 8, `commit`) | `8efdaa65e` |
| tools/demo | offline signer, agent-ctl command, mock ledger, self-contained capability demo | `examples/{sign-fragment,mock-ledger,fragment-demo}.rs`; `agent-ctl` | `392d890a8`, `69228f3b5`, `a63b9d5b3` |
| docs | in-tree guide + this reference | `docs/cc/{parma-hardening-features,fr1-fragment-e2e,fr1-fragments}.md`, `kata-opa/fragment-demo.rego` | `adaa7558b`, `d8149983e`, `b65ba9113`, `26d013c95`, `5dc0744f9` |

---

## 3. Verification pipeline (order of gates)

Every fragment is verified by a chain of fail-closed gates before it is committed. Both the
native detached-signature path (`verify`), the COSE path (`verify_cose`), and the did:x509
path (`verify_cose_x509`) converge on the shared `check_gates`:

1. **Issuer identity + signature.** Either (a) the issuer is in the measured authorized set and
   an Ed25519 signature verifies over the canonical statement, or (b) a did:x509 chain in the
   COSE `x5chain` path-validates to a measured CA (each issuer cert `CA:TRUE`, in-date),
   satisfies the did:x509 policy (subject CN / EKU / DNS SAN), is not revoked, and the leaf key
   signs the statement — and the derived did equals the declared issuer. No downgrade: an
   x5chain-bearing envelope is always verified as did:x509.
2. **Feed declared** (FR-1e) — `(issuer, feed)` must be an accepted pair.
3. **Monotonic SVN** (FR-1b/1e/1i) — `svn ≥ max(declared floor, persisted high-water + 1)`.
4. **Transparency receipt** (FR-1f) — if required for the scope: a Stage-1 detached ledger
   signature and/or a Stage-2 inclusion+consistency proof, from an `allowed_ledgers` ledger,
   verified against a current trust-list key; Stage 2 also requires the signed tree head to be
   an append-only extension of the last-seen head (monotonic, consistency-proven).
5. **Composition** (FR-1g) — every `requires` id must already be loaded.
6. **Add-only** — a fragment may only add grants; it may never introduce a grant that relaxes
   a declared root constraint.
7. **Ordering** (FR-1j) — in ordered mode, the fragment's *signed* `prev_log_head` must equal
   the store's current rolling head; the head then advances by hashing the statement in.

The statement (`signing_bytes`, `kata-policy-fragment/v3`) binds issuer, feed, SVN, sorted
grants, module, sorted includes, sorted requires, **and** `prev_log_head` — so none of these,
including the asserted predecessor, can be altered without invalidating the signature. The
receipt/ledger id and the transparency proof are countersignatures/assertions *over* that
statement and are deliberately outside it.

---

## 4. Capability detail

### 4.1 Core: signed, authorized, add-only, monotonic (FR-1a/1b/1c)
`FragmentStore` (in `fragments.rs`) is the verifier and add-only accumulator. `authorize_issuer`
registers a measured Ed25519 key; `set_min_svn`/`declare_feed` set floors; `verify` runs the
gates without mutating; `commit` advances SVN/grants; `load` = verify+commit. `apply_fragment_module`
(`policy.rs`) merges a verified module additively and refuses a package outside its `includes`
namespace. Fail-closed: with no authorized issuers, every fragment is rejected.

### 4.2 did:x509 issuer identity (FR-1d)
`did_x509.rs` parses the COSE `x5chain` (header 33), path-validates leaf→CA with per-cert
signature verification and validity windows, **requires every issuer cert to assert
`basicConstraints CA:TRUE`** (so a non-CA leaf cannot mint sub-certs), enforces the did:x509
policy over the leaf, rejects revoked fingerprints, and verifies the COSE signature with the
leaf key. **Multi-algorithm (BL-2):** the leaf COSE signature dispatches on the envelope's
declared algorithm and chain-link signatures on each certificate's `signatureAlgorithm` OID —
**ES256 (P-256), ES384 (P-384), RS256 and PS256 (RSA)** are supported (shared `cose_keys.rs`
verifier), with no cross-algorithm confusion (the algorithm must match the key type). Trust is
anchored on **CA fingerprint + policy**, so leaf **rotation** — and choice of leaf algorithm —
needs no config change. Pure-Rust RustCrypto (`x509-cert`, `p256`, `p384`, `rsa`); no Go.
Errors: `InvalidCertChain`, `UntrustedCa`, `DidX509Mismatch`, `RevokedCertificate`,
`CertExpired`.

### 4.3 Transparency receipts + Trust List (FR-1f)
Receipts prove a fragment's issuance is publicly auditable. The store holds a
`transparency_trust_list` (ledger id → current key set; multiple keys ⇒ rotation), per-scope
`allowed_ledgers`, and per-scope `required_receipt_from`. The receipt gate selects the ledger
(untrusted id, safe because verification only passes if that ledger's key actually signed),
enforces the allow-list and required ledger, and verifies against any current key.
**Multi-algorithm (BL-2):** each ledger key carries its algorithm, so receipts and signed
tree heads may be Ed25519, ES256, ES384, PS256, or RS256 (shared `cose_keys.rs` verifier).
Errors: `MissingReceipt`, `InvalidReceipt`, `LedgerNotAllowed`, `ReceiptFromDisallowedLedger`.
A single legacy anchor maps to a default ledger (back-compat).

### 4.4 Transparency-log inclusion + consistency — Stage 2 (FR-1f Stage 2)
`merkle.rs` implements RFC 6962 leaf/node hashing, inclusion-proof root recomputation, and
consistency-proof verification (property-tested across sizes 1..33 and every index/prefix). A
`kata-ttl-proof/v1` receipt carries a signed tree head (size, root, ledger signature), an
inclusion proof of the statement, and an optional consistency proof. The gate verifies the STH
signature against a trust-list key, verifies the statement's inclusion at that head, and
requires each head to be an **append-only extension** (consistency-proven) of the last-seen
head for that ledger. The per-ledger tree head is persisted **raise-only** so the log cannot be
rewound across restart. Errors: `InvalidInclusionProof`, `LogRolledBack`.

### 4.5 Feed scoping + composition (FR-1e / FR-1g)
Feeds partition an issuer's fragments; the base declares accepted `(issuer, feed)` pairs and
their SVN floor, and SVN is monotonic *per pair*. `requires` lets a fragment depend on
already-loaded fragments (identified by `issuer/feed/svn`), so composition is explicit and
cycle-free by construction.

### 4.6 COSE_Sign1 envelope (FR-1h)
`verify_cose` accepts a COSE_Sign1 (CBOR) envelope whose payload equals the statement, verified
via the pure-Rust `coset` crate — interop with standard COSE tooling with no Go dependency. The
did:x509 path (FR-1d) rides inside the same envelope via `x5chain`.

### 4.7 SVN persistence (FR-1i)
`export_svn_state`/`import_svn_state` persist the per-`(issuer,feed)` high-water marks (and the
FR-1j ordering head + FR-1f-Stage-2 tree heads) to sealed/encrypted-scratch storage. Import is
**raise-only**, so an agent/VM restart can never reopen a rollback window.

### 4.8 Append-only application ordering (FR-1j)
Each fragment's *signed* statement binds `prev_log_head`; in ordered mode the guest requires it
to equal its current rolling head, then advances `head = sha256(head ‖ sha256(statement))` on
commit. Because the predecessor head is signed by the issuer, the untrusted delivery path
cannot forge order — a reordered, omitted, or inserted fragment presents the wrong predecessor
and is rejected (`LogHeadMismatch`). `export_fragment_log` yields a deterministic,
customer-auditable record of the exact applied sequence; the head persists raise-only across
restart.

---

## 5. Measured configuration

Seeded at boot by `main.rs::seed_fragment_trust_root` from a measured-rootfs file
(`/etc/kata/fragment-issuers.toml`, override `KATA_FRAGMENT_ISSUERS`); absent config ⇒ no
authorized issuer ⇒ fail-closed. Shape:

```toml
require_receipt = true                 # global default
ordered = true                         # FR-1j append-only ordering
# log_genesis_hex = "<hex>"            # optional; default measured constant
require_x509 = false                   # FR-1d: when true, all fragments must carry a valid x5chain
# revoked = ["<sha256-hex>", ...]      # FR-1d revocation list

[[ledger]]                             # FR-1f transparency trust list
id = "acl"
pubkey_hex = ["<64 hex>", "<rotated>"] # Ed25519 keys; multiple ⇒ rotation
  [[ledger.key]]                       # BL-2: non-Ed25519 ledger key (ES256/ES384/PS256/RS256)
  alg = "es256"
  spki_hex = "<SubjectPublicKeyInfo DER, hex>"

[[ca_anchor]]                          # FR-1d did:x509 anchor
did = "did:x509:0:demo-ca:issuerX"
ca_fingerprint_hex = "<sha256 of CA DER>"
require_eku = ["1.3.6.1.5.5.7.3.3"]

[[issuer]]                             # FR-1b raw-Ed25519 issuer
id = "issuerA"
ed25519_pubkey_hex = "<64 hex>"
min_svn = 0
required_receipt_from = ["acl"]        # FR-1f per-issuer required receipts
  [[issuer.feed]]                      # FR-1e named feed
  name = "prod"
  min_svn = 0
  allowed_ledgers = ["acl"]
  required_receipt_from = ["acl"]
```

Runtime SVN/ordering/tree-head state is persisted to `/run/kata/fragment-svn.state`
(override `KATA_FRAGMENT_SVN_STATE`) and re-imported raise-only at boot.

---

## 6. Tooling, tests, and proofs

**Tooling** (`src/agent/security-reference-monitor/examples/`, `src/tools/agent-ctl/`):
- `sign-fragment` — Ed25519 keygen + signer; emits detached sig, COSE_Sign1 (`--cose`),
  did:x509 ES256 envelopes (`--x509-key/--x509-chain`), ledger-tagged receipts (`--ledger`),
  ordering (`--prev-head`), statements for a ledger (`--emit-statement`); `verify-x509`
  offline verifier.
- `mock-ledger` — RFC 6962 transparency-log stand-in emitting `kata-ttl-proof/v1` proofs.
- `agent-ctl LoadPolicyFragment` — key=value args (`issuer= svn= feed= includes= requires=
  receipt= receipt_ledger= prev_head= proof= module= sig= cose=`).
- `fragment-demo` — offline, no-cluster/no-openssl demo asserting every capability.

**Unit tests** — fragment/identity tests (of 98 in the SRM crate): `fragments::tests::*`
(core, feed, receipt/trust-list/rotation incl. an ES256 ledger key, chaining, persistence,
COSE, ordering, Stage-2 inclusion/consistency), `did_x509::tests::*`
(valid/untrusted/broken/expired/revoked/rotated/policy-mismatch/intermediate-CA/
non-CA-intermediate + **ES384 and RSA-RS256 end-to-end**), `cose_keys::tests::*` (alg mapping,
cross-alg rejection), `merkle::tests::*` (inclusion + consistency across sizes).

**Live E2E** (strict `kata-parma`, over vsock) — `fr1-fragment-attack.sh` (deny→load→allow),
`fr1-cose-attack.sh` (COSE), `fr1-x509-attack.sh` (did:x509 valid/untrusted/revoked/rotated),
`fr1-ordering-attack.sh` (out-of-order rejected), `fr1-ttl-attack.sh` (inclusion+consistency
accepted, rewound-log + missing-proof rejected). Aggregated as `negative-matrix.sh` stages.

---

## 7. Net guarantee

Unsigned, wrong-issuer, untrusted-CA, revoked, expired, non-CA-intermediate, rolled-back,
undeclared-feed, over-broad, missing/invalid/disallowed-receipt, out-of-order, or
unsatisfied-requirement fragments are all rejected fail-closed. Accepted fragments extend the
live policy additively, in a verifiable order, with an auditable record — never relaxing a base
invariant and never reopening a rollback window, even across a restart.
