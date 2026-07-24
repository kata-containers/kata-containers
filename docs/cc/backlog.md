# Execution-integrity backlog — remaining work

Everything in the feature baseline and the post-baseline hardening set is **merged** to
`coco-parity` and documented in `docs/cc/parma-hardening-features.md` (the single source of
truth for shipped features). This file tracks only what is **not yet complete**: open work
items and the merged features whose *live* end-to-end validation is deployment-time.

_For the full list of shipped items and their commits/PRs, see
`docs/cc/parma-hardening-features.md`._

## Open work items

_None._ All planned execution-integrity work items (BL-1…BL-9) are merged to `coco-parity`.
BL-5 (bind measured state into initdata) landed in PR #10 (branch `bl5-initdata-measured`) — see
`parma-hardening-features.md` §"Measured-initdata trust roots". What remains is **live
validation** of already-merged features (below).

## Merged, but live end-to-end validation is deployment-time

These features are implemented, unit-tested, and build-clean on `coco-parity`; the remaining
validation needs a running node, an OCI registry, or a live external ledger — none of which
exist inside the confidential-guest test bed. They are **not** code gaps.

| Feature | What still needs a live environment |
|---|---|
| FR-4C — verified read-only layers | `devicemapper` agent build + a GPT/EROFS dm-verity image to exercise the gate on real hardware; optional dm-table read-back of the effective root hash. |
| FR-4D — verified guest-pull images | A guest-pull-enabled (CDH) agent + pod that attempts an unlisted/tag-only image and is denied. |
| FR-1f Stage 2 — external SCITT/CCF receipts | A reachable SCITT/CCF endpoint (e.g. Azure Confidential Ledger) to feed a real `kata-ccf-proof/v1` receipt end-to-end. |
| FR-1 delivery — boot-time OCI fragment pull | A dev OCI registry preloaded with GOOD/badsvn/wrongiss/tampered fragment artifacts, to assert GOOD injects and the rest abort the VM. |
| `genpolicy-fragmentgen` packaging/push | A reachable OCI registry to validate `--push` against (packaging + settings emission are verified offline). |

## Deferred / out of scope

See `docs/cc/parma-hardening-features.md` §"Deferred / out of scope" for FR-13
(snapshot/restore/migration sealing — not applicable to GPU-passthrough CC), the
hardware-gated TEE/GPU-attestation items, and the optional FR-10 content-addressed
artifact API.
