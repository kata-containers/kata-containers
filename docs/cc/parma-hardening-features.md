# Confidential-runtime execution-integrity hardening (PARMA compliance)

In order to support **PARMA compliance**, we identified a set of hardening features that
close concrete execution-integrity gaps in the Kata confidential-containers runtime, and
this development branch (`coco-parity`) addresses them.

PARMA reasons about a guest whose *authorized* plan equals its *executed* plan under a
closed-door policy that mediates every host-reachable operation. Reaching that bar
requires more than a policy check on the incoming request: the agent must default to
deny, treat host-supplied identifiers as untrusted aliases, bind every mutating operation
to a transactional state machine, verify that the object actually executed matches the
object that was authorized, and freeze or refuse surfaces that would otherwise let the
host mutate a running workload. The features below implement those properties as a
"strict" build of the guest agent (`STRICT_POLICY=yes`), deployed via the `kata-parma`
runtime profile.

This document maps each feature to the requirement it satisfies, the commits that
implement it, the security guarantee it introduces, and how it was validated.

---

## How to read this document

- **Branch:** `coco-parity` (fork), based on kata `main` @ `4984d7944`.
- **Strict build:** all hardening is compiled under the `strict-policy` feature; a normal
  (non-strict) build is behaviourally unchanged, so the branch is safe to carry as a
  superset.
- **Trusted-state authority:** most guarantees are enforced by an agent-internal
  **Security Reference Monitor (SRM)** crate (`src/agent/security-reference-monitor/`),
  which owns the transactional lifecycle, occurrence registry, resource graph, CDI trust,
  policy-fragment verifier, scratch classifier, TOCTOU handle binding, and network-phase
  machine. Keeping this logic in one crate makes it unit-testable and, for the lifecycle,
  formally model-checkable.
- **No new host↔guest protocol** except FR-1 (`LoadPolicyFragment`), which is additive and
  backward-compatible.
- **Validation vocabulary:** *unit* = crate unit/integration tests; *matrix* = the live
  `policy-matrix.sh` on strict `kata-parma` pods (expected 5/5); *live attack* = a
  `kata-agent-ctl` ttRPC client impersonating the shim against a running guest.

---

## Feature → requirement → commit map

| Requirement | Feature | Key commits |
|---|---|---|
| FR-2 | Closed-door default policy (fail closed) | `0a538d111` |
| FR-12 | One-shot policy activation + strict capability advertisement | `85b3ce3f7`, `ad01dd311`, `8424e7e08` |
| FR-6 | Two-phase transaction manager (SRM substrate) | `b10ffc663`, `b88ff8e51`, `e4d6c8c97`, `dfac4bd7a` |
| FR-3 | Canonical object: authorized == executed | `61ee0ca0d`, `5a736c4a8`, `798301421` |
| FR-9 | Container occurrence + lifecycle state machine | `96a0d641c`, `2434d3ef2` |
| FR-7 | Complete-mediation manifest + CI coverage | `d68c96708` |
| FR-4A | Ordered bijective resource graph | `6f8f42eea` |
| FR-11 | Trusted device/CDI resolution + occurrence binding | `9669a913b`, `0f3aa0f2f` |
| FR-10 | Disable generic CopyFile in strict | `0b41cf8a4` |
| FR-1 | Signed, add-only policy fragments (feed scoping, receipts, chaining, COSE, applied to live engine) | `11285337c`, `4ccd43f8a`, `bf602cb18`, `dd2630053`, `294353a2a`, `ff8a4d5b9`, `c6b52c2ba`, `69228f3b5`, `c0ea3cb25`, `f7ed23319`, `93e1ff6e5`, `392d890a8`, `adaa7558b` |
| FR-5 | Encrypted scratch by effective mode | `44d6f9d04`, `b1603c3a6` |
| FR-4B | Mount bound to the checked handle (TOCTOU) | `44d6f9d04`, `dbea0d59b` |
| FR-14 | Network phase binding | `44d6f9d04`, `8cf9c5785` |
| FR-7 (rest) | Debug console + diagnostics disabled in strict | `8cf9c5785` |
| FR-15 | Formal model + fault injection + equivalence-claim proof | `21ac6e048`, `e76bc8d81` |
| FR-8 | Structured, no-leak decision objects | `a59f5e74f` |
| — | Local dev-env build plumbing | `c486222c1` |

---

## Stage 1 — Strict profile foundation

### FR-2 — Closed-door default policy
- **Gap:** a guest with no (or a not-yet-delivered) policy would fail open, allowing host
  requests before an authorized policy is active.
- **Fix:** the strict build ships a closed-door default policy that denies every request
  except `SetPolicyRequest`. The "ignore requests failing policy" escape hatch is compiled
  out of strict builds.
- **Guarantee:** no host-reachable operation is permitted before an authorized policy is
  activated; unknown/undefined requests are denied, not allowed.
- **Commit:** `0a538d111`.
- **Validated:** matrix — a pod booted with no policy is closed-door (sandbox denied).

### FR-12 — One-shot policy activation + capability advertisement
- **Gap:** if the host can replace the active policy at runtime, it can weaken enforcement
  after attestation; a verifier also needs to distinguish a strict guest from a permissive
  one.
- **Fix:** once an authorized policy is active, `SetPolicy` is refused (changing policy
  requires a new verifier-authorized epoch). The guest advertises `strict-policy` in its
  build features.
- **Guarantee:** policy is immutable within an epoch; a shim/verifier can detect a strict
  guest before relying on it.
- **Commits:** `85b3ce3f7` (one-shot), `ad01dd311` (advertisement), `8424e7e08` (build).
- **Validated:** matrix + capability advertisement observed live.

---

## Stage 2 — Canonical object + transaction core

### FR-6 — Two-phase transaction manager (Security Reference Monitor)
- **Gap:** policy state and runtime state could diverge on partial failure, leaving the
  enforcer believing a container/mount/identity exists (or not) when the opposite is true.
- **Fix:** a universal `ReferenceMonitor` models every mutating operation as
  `prepare → execute → commit`/`abort`, with idempotent replay, anti-replay via a monotonic
  state version, and a fail-closed `quarantine`. `CreateContainer`, `ExecProcess`, and
  `SignalProcess` run as SRM transactions; policy state is snapshotted before authorization
  and restored on abort.
- **Guarantee:** policy and runtime state commit together or are reconciled/rolled back;
  an unprovable state quarantines the monitor (never fails open).
- **Commits:** `b10ffc663` (crate), `b88ff8e51` (create), `e4d6c8c97` (exec/signal),
  `dfac4bd7a` (policy-state rollback).
- **Validated:** unit (transaction manager tests) + matrix no-regression.

### FR-3 — Canonical object (authorized == executed)
- **Gap:** the agent mutates the authorized request before executing it (effective signal
  resolution, PCI-address rewriting of the exec environment, and a chain of in-guest OCI
  transformers at create time), so the executed object was not the object policy saw.
- **Fix, at all three mutation sites:**
  - **Effective signal:** the delivered signal is resolved (e.g. `SIGTERM`→`SIGKILL` for an
    init process with no handler) *before* authorization, so policy authorizes the signal
    actually delivered (`61ee0ca0d`).
  - **Exec environment:** `update_env_pci` is applied before authorization so the policy
    evaluates the environment actually given to the process (`798301421`).
  - **Create spec:** the authorized OCI spec is digested before any transformer runs, and
    the fully-resolved spec is digested and bound to the create transaction; divergence is
    recorded for audit (`5a736c4a8`).
- **Guarantee:** the object that executes is explicitly and auditably tied to the object
  that was authorized.
- **Commits:** `61ee0ca0d`, `798301421`, `5a736c4a8`.
- **Validated:** unit + matrix no-regression.

---

## Stage 3 — Resource graph + occurrence + total mediation

### FR-9 — Container occurrence + lifecycle state machine
- **Gap:** the host-supplied `container_id` is an untrusted alias — it can be forged,
  reused, or replayed to drive illegal lifecycle transitions.
- **Fix:** the enforcer mints its own occurrence handle per container and drives it through
  `created → running → stopped → removed`. Lifecycle RPCs are gated on occurrence state,
  with a monotonic per-alias generation as a replay guard and optional per-declaration
  cardinality.
- **Guarantee:** start-before-create, exec/signal on an unknown or not-running occurrence,
  operations on a removed occurrence, and replay of a stale generation are all rejected.
- **Commits:** `96a0d641c` (registry), `2434d3ef2` (wiring).
- **Validated:** unit + **live attack** — `StartContainer` on a never-created id and
  `SignalProcess` on an unknown id are denied under an allow-all policy (the gate is the
  occurrence machine, not the policy).

### FR-7 — Complete-mediation manifest
- **Gap:** without a machine-checked inventory, a newly added RPC could ship unmediated.
- **Fix:** a manifest classifies every agent ttRPC method by its enforcement point; build
  tests fail if the proto and manifest drift, if the manifest lists a removed method, or if
  a mediated handler does not reach its enforcement point.
- **Guarantee:** every host-reachable RPC is provably mediated; there is no always-allow
  escape hatch (the strict default is closed-door).
- **Commit:** `d68c96708`.
- **Validated:** three build-time CI tests.

### FR-4A — Ordered bijective resource graph
- **Gap:** verifying only that *some* declared resource matches each presented one, with
  equal counts, lets image layers be reordered or a duplicate satisfy one declaration
  twice — producing a different root filesystem than authorized.
- **Fix:** a typed verifier enforces an order-relevant 1:1 mapping between declared and
  presented resources and equality of each resource's integrity digest (dm-verity root
  hash), returning typed handles bound to the declaration index.
- **Guarantee:** reorder, duplicate, undeclared/extra, cardinality mismatch, and
  stale-digest substitution are all rejected.
- **Commit:** `6f8f42eea`.
- **Validated:** unit tests (reorder / duplicate / stale-verity / undeclared / cardinality).
- **Follow-up:** moving this bijection into the live rego/genpolicy storage check needs a
  dm-verity/guest-pull-backed image to validate.

---

## Stage 4 — Conditional capabilities

### FR-11 — Trusted device / CDI resolution
- **Gap:** CDI resolution applies `containerEdits` (env/devices/mounts/hooks) from spec
  files in the guest *after* authorization, from a possibly host-influenced source — the
  device instance of the canonical-object gap.
- **Fix:** every CDI spec that provides a requested device must be **measured** (its content
  digest present in an authorized set); resolution is closed-door by default (host-arbitrary
  CDI refused), and each authorized device is bound to the container occurrence.
- **Guarantee:** a host cannot smuggle privilege via an unmeasured CDI annotation or spec;
  resolved device handles are tied to the occurrence.
- **Commits:** `9669a913b` (authorization logic), `0f3aa0f2f` (agent wiring + binding).
- **Validated:** unit + device-module tests; matrix no-regression.
- **Deferred (HW):** real GPU CC-attestation evidence for the device.

### FR-10 — Disable generic CopyFile in strict
- **Gap:** a generic host→guest `CopyFile` lands host-chosen bytes at a host-chosen path
  with no content-addressing or execution-integrity guarantee.
- **Fix:** strict builds refuse `CopyFile` outright (independent of the active policy) and
  advertise `no-generic-copyfile`.
- **Guarantee:** the host cannot plant files the policy never authorized.
- **Commit:** `0b41cf8a4`.
- **Validated:** **live attack** — `CopyFile` under an allow-all policy returns
  `PERMISSION_DENIED`; matrix no-regression (pod creation does not require CopyFile).

### FR-1 — Signed, add-only policy fragments
- **Gap:** the base policy is one-shot/immutable within an epoch (FR-12), but some
  deployments need to *extend* what a workload may do at runtime without a new attestation.
  Doing so is unsafe unless every extension is authenticated, monotonic, scope-limited, and
  incapable of relaxing a base invariant.
- **Fix:** a fragment carries a **signed Rego module** (the statement binds issuer, feed,
  SVN, grants, includes, requires, and module). It is verified and, on success, **merged
  into the live policy engine** so it changes authorization at enforcement time — via an
  additive, namespace-scoped `add_policy` that never touches the one-shot `set_policy` lock.
  Verification is a chain of fail-closed gates:
  - **authorized issuer** (Ed25519), signature over the statement;
  - **feed scoping** — the base declares accepted `(issuer, feed)` pairs; an undeclared feed
    is rejected;
  - **strictly-monotonic per-`(issuer, feed)` SVN**, with a declarative floor from measured
    state, persisted across restart (import can only raise the floor, never lower it);
  - **transparency Trust List** — receipts are validated at runtime against a measured
    trust list of ledgers (each with rotatable keys); per-`(issuer, feed)` `allowed_ledgers`
    scoping and policy-driven `required_receipts` decide which ledger(s) a receipt must come
    from (a single legacy anchor maps to a default ledger for back-compat). Receipts may be a
    detached ledger signature or a **transparency-log inclusion + consistency proof** (RFC
    6962 Merkle): the fragment must be provably recorded in an append-only log whose signed
    tree head only ever grows (a rewound/forked log is rejected; the head is persisted
    raise-only across restart);
  - **issuer identity** — either a pinned Ed25519 key **or** a `did:x509` certificate chain
    in the COSE `x5chain` header: path-validated to a measured CA fingerprint, leaf must
    satisfy a `did:x509` policy (subject CN / EKU / DNS SAN), revoked fingerprints rejected;
    trust anchored on **CA + policy** so leaf rotation needs no config change; `require_x509`
    disables the raw-key path (no downgrade). Pure-Rust X.509/ECDSA; no Go dependency;
  - **add-only / includes scoping** — a module may only contribute in its declared
    `agent_policy.fragments[.<include>]` namespace and can never redefine a base rule;
  - **composition** — a fragment may `require` other fragments, which must already be loaded
    (cycles/unbounded depth are impossible by construction).
  - **append-only application ordering** (opt-in) — a rolling, signed log head binds each
    fragment to its predecessor (bound into the signed statement), so reordering, omission,
    or insertion is rejected fail-closed; the head is persisted raise-only across restart and
    the ordered log is exportable as a non-repudiable, customer-auditable record of the exact
    applied sequence.
  The `LoadPolicyFragment` RPC is doubly mediated (policy `is_allowed` + fragment
  verification) and fail-closed (no authorized issuers ⇒ every fragment rejected). Verify →
  apply → commit is atomic. Both a native detached-Ed25519 signature and a **COSE_Sign1
  (CBOR) envelope** are accepted (COSE via the pure-Rust `coset` crate; no Go dependency).
  Issuers, feeds, SVN floors, the transparency trust list, the did:x509 CA anchors, and the
  ordering genesis are all configured from measured state.
- **Guarantee:** only signed, non-rolled-back, scope-limited policy extensions from an
  attested-trusted issuer (pinned key or did:x509 chain) can change what the workload may do
  — in a verifiable order, auditably; unsigned, wrong-issuer, untrusted-CA, revoked,
  rolled-back, undeclared-feed, over-broad, invalid/disallowed-receipt, out-of-order, or
  unsatisfied-requirement fragments are all rejected.
- **Commits:** `11285337c`,`4ccd43f8a` (verifier + RPC); `bf602cb18`,`dd2630053`,`294353a2a`
  (Iteration 1: apply-to-live-engine, attested trust root, structured payload);
  `ff8a4d5b9`,`c6b52c2ba`,`69228f3b5` (Iteration 2: feed scoping, cryptographic receipts,
  chaining); `c0ea3cb25`,`f7ed23319`,`93e1ff6e5` (Iteration 3: SVN persistence, COSE_Sign1);
  `db24d40f5` (Iteration 4: transparency trust list), `9cddd7f75` (did:x509 identity),
  `8efdaa65e` (append-only ordering), `a63b9d5b3` (capability demo), Stage 2
  transparency-log inclusion + consistency proofs (RFC 6962 Merkle);
  `392d890a8`,`adaa7558b` (signer example, agent-ctl command, demo policy, guide).
- **Validated:** 84 SRM unit tests (issuer/signature/SVN/feed/receipt/trust-list/rotation/
  did:x509-chain/revocation/includes/chaining/persistence/COSE/ordering/Merkle-inclusion/
  consistency); an offline, self-contained capability demo (`examples/fragment-demo` —
  asserts all of the above with no cluster/openssl); **live E2E** — a base-denied exec
  becomes allowed only after a valid signed fragment is loaded over vsock
  (`fr1-fragment-attack.sh`), again via a COSE_Sign1 envelope (`fr1-cose-attack.sh`), via a
  did:x509 chain (`fr1-x509-attack.sh`); an out-of-order fragment is rejected
  (`fr1-ordering-attack.sh`); and a fragment without a transparency-log inclusion+consistency
  proof, or one presenting a rewound log, is rejected (`fr1-ttl-attack.sh`). Reproducible dev
  guide: `docs/cc/fr1-fragment-e2e.md`.
- **Follow-up (optional):** additional signature algorithms (RSA/ES384) for did:x509 leaves
  and receipts behind the existing verification entry points; binding the issuer config +
  SVN/ordering/tree-head state into the initdata measured section proper.

---

## Stage 5 — Production hardening

### FR-5 — Encrypted scratch by effective mode
- **Gap:** trusting the host's storage driver options to decide whether scratch is
  encrypted lets a host claim encryption while presenting a plaintext backing device.
- **Fix:** the enforcer classifies scratch by its **effective** device-mapper target stack
  (`dmsetup table`) — `crypt`/`integrity` — not the host's claim, and refuses a scratch
  mount whose effective stack is plaintext.
- **Guarantee:** writable scratch is provably encrypted; host-claims-encrypted-but-plaintext
  is denied.
- **Commits:** `44d6f9d04` (classifier), `b1603c3a6` (wiring).
- **Validated:** unit tests (classification / plaintext-denied / effective-not-claimed).
- **Follow-up:** live block-`emptyDir` validation needs a dm-crypt emptyDir pod.

### FR-4B — Mount bound to the checked handle (TOCTOU)
- **Gap:** a mount destination validated at check time can be swapped (symlink/rename)
  before the mount syscall uses it.
- **Fix:** capture the destination's identity (dev/ino) right after validation and
  re-verify it immediately before `baremount`; a swap is detected and the mount refused.
- **Guarantee:** a mount binds to the object that was checked, not a re-resolved name.
- **Commits:** `44d6f9d04` (handle-binding), `dbea0d59b` (wiring).
- **Validated:** unit tests including a real filesystem swap; matrix no-regression.

### FR-14 — Network phase binding
- **Gap:** a host that can add a route, rewrite iptables, or spoof ARP *after* the workload
  starts can exfiltrate or redirect traffic.
- **Fix:** a phase machine (`Boot → SandboxSetup → WorkloadRunning → Locked`) permits
  network-mutating RPCs only during sandbox setup and freezes them once the workload runs;
  a route allowlist further constrains programmed destinations.
- **Guarantee:** post-start network mutation is refused.
- **Commits:** `44d6f9d04` (phase machine), `8cf9c5785` (wiring).
- **Validated:** unit tests + **live attack** — `UpdateRoutes` on a running pod is denied
  (`FrozenPhase`); matrix no-regression (network config during sandbox setup is unaffected).

### FR-7 (remainder) — Strict runtime surface
- **Gap:** the interactive debug console and guest diagnostics are un-mediated
  guest-access / data-exfiltration surfaces.
- **Fix:** strict builds never launch the debug console (regardless of host config) and
  refuse `GetDiagnosticData`; the guest advertises `no-debug-console` and
  `no-guest-diagnostics`.
- **Guarantee:** no un-mediated shell or diagnostic dump in a strict guest.
- **Commit:** `8cf9c5785`.
- **Validated:** **live** — features advertised; `GetDiagnosticData` denied in strict.

---

## Stage 6 — Formal proof + auditability

### FR-15 — Formal model, fault injection, and the equivalence-claim proof
- **Goal:** prove that no reachable state of the monitor is permissive or phantom — the
  equivalence claim underpinning PARMA-style reasoning.
- **Fix:**
  - a **TLA+ model** (`src/agent/security-reference-monitor/formal/SRM.tla`) of the
    two-phase lifecycle + quarantine, model-checked by TLC over all reachable states:
    version equals committed count (no phantom/missed commit), `Commit` is enabled only from
    `executed` (authorized == executed), committed/aborted are terminal & exclusive, and
    quarantine is sticky;
  - **fault-injection + fuzz** tests
    (`src/agent/security-reference-monitor/tests/fault_injection.rs`): a fault injected at
    every phase and reconciled as the agent does never leaves an operation committed or
    advances the version; a deterministic 200-seed fuzz checks the invariants after every
    step;
  - an **aggregate negative-test runner** that runs the policy matrix and the FR-9/FR-10/
    FR-14 live attacks (plus the unit/fault tests and the model check) as one gate.
- **Guarantee:** the lifecycle safety properties hold under all interleavings and injected
  faults; the negative-test matrix is the reproducible equivalence-claim proof.
- **Commits:** `21ac6e048` (fault/fuzz), `e76bc8d81` (TLA+ model).
- **Validated:** TLC — *no error* over 250 states; fault/fuzz tests pass; aggregate runner
  green.

### FR-8 — Structured, rule-attributable decision objects
- **Gap:** denials must be auditable without leaking workload data.
- **Fix:** on denial the policy emits a `DecisionObject` recording the endpoint, the
  decision, the denied Rego rule (query path), and the **names** of the request's top-level
  fields — never their values.
- **Guarantee:** denials are rule-attributable and carry no environment values, sealed
  secrets, or policy text.
- **Commit:** `a59f5e74f`.
- **Validated:** unit tests for attribution and the no-secret-leakage guarantee.

---

## Scope relative to the upstream baseline

This branch targets the **upstream** Kata base it forks from. Some deployments already
harden a subset of these areas through build configuration or product-layer mechanisms; to
keep the guarantees explicit and portable, the features below are classified by how they
relate to the upstream baseline. All remain valid on an unmodified upstream base; on a
pre-hardened deployment a few are parity or additional defense-in-depth.

- **Baseline-independent invariants (novel relative to the upstream default):**
  - **FR-2** — the upstream rootfs default policy is fail-open (`allow-all.rego`). A
    deny-all-except-`SetPolicy` policy file exists in-tree but is not the default. This
    branch compiles the **closed-door default into the strict agent binary** so it does not
    depend on build-time policy-file selection, and it **compiles out** the
    `AllowRequestsFailingPolicy` escape hatch entirely. This is a stronger, build-independent
    form of the closed-door posture.

- **Defense-in-depth (the base capability may already exist; these add assurance):**
  - **FR-5 (effective-mode scratch verification)** — encrypted ephemeral storage already
    exists when requested via storage driver options; this branch additionally verifies the
    **effective** device-mapper stack (so a plaintext effective mount is refused even when
    encryption was requested) and enforces a mandatory-encryption invariant.
  - **FR-3 (create-spec canonical binding)** — because policy evaluation, the in-guest
    transformers, and execution all run inside a **single trusted agent process** sourcing
    trusted guest state, byte-identity between the authorized and executed OCI object is not
    required for the security property. This branch therefore **only records/audits** the
    authorized→executed digest relationship (it does not reorder transformers or enforce
    byte-identity). The **effective-signal** and **exec-environment** pre-authorization
    resolution (also under FR-3) are independent integrity improvements and are enforced.
  - **FR-4B (mount TOCTOU handle binding)** — a defensive re-verification of the mount
    destination's identity; closes a check-to-use window rather than a demonstrated exploit.

- **Confirmed structural gaps closed here (independent of any product-layer hardening):**
  FR-4A (ordered/bijective resource graph), FR-9 (occurrence/cardinality), FR-1 (signed
  policy fragments), FR-6 (universal transactional rollback), FR-7 (total-mediation
  manifest + gating the always-allowed lifecycle RPCs), FR-11 (trusted CDI/device
  resolution), FR-14 (network phase binding + route allowlist), FR-10 (CopyFile content),
  and FR-8/FR-15 (auditability + the model-checked equivalence proof). These are not
  addressed by image-integrity or default-posture hardening alone.

---

## Deferred / out of scope

- **FR-13 (snapshot/restore/migration sealing) — not applicable.** Snapshot, restore, and
  live-migration are not possible for GPU-passthrough (VFIO) confidential workloads at the
  hypervisor/device layer, so there is no state to securely restore. The strict guest should
  advertise these as unsupported and deny them; the anti-replay defenses that would back
  secure migration (monotonic SRM state version, occurrence generation) already exist and
  are model-checked. The sealing machinery itself is not built.
- **Hardware-gated items** requiring a real TEE (SNP/TDX) or real GPU attestation:
  verifier-bound claims and secret-release gating (part of FR-12), and real GPU
  CC-attestation evidence for FR-11. These cannot be exercised on a software-only bed.
- **FR-10 content-addressed artifact API** (`BeginArtifactInstall/…`) — an optional
  alternative to the "disable CopyFile" default that this branch ships; build it only if
  trusted host-delivered artifacts become a requirement.

---

## Validation at a glance

- **Unit / integration:** the SRM crate carries the transaction manager, occurrence
  registry, resource graph, CDI trust, fragment verifier, scratch classifier, handle
  binding, network-phase machine, and lifecycle fault-injection/fuzz tests, all green.
- **Formal:** TLC model-checks the lifecycle safety properties with no error.
- **Live matrix:** the strict `kata-parma` profile passes the policy-enforcement matrix
  with no regression, and the FR-9/FR-10/FR-14 live ttRPC attacks are denied.
- **Mediation CI:** build-time tests keep the complete-mediation manifest in sync with the
  agent protocol.
