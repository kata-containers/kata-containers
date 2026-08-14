# Confidential persistent block storage

## Status

Proposed in [kata-containers/kata-containers#13638][proposal-issue], following
the feature request in [kata-containers/kata-containers#8515][issue-8515]. The
implementation in [kata-containers/kata-containers#13637][implementation-pr]
remains a draft until the community agrees on the contract and ownership
boundaries.

## Problem

Confidential Containers can create encrypted ephemeral volumes, but there is no
upstream contract for reopening encrypted persistent storage. A workload that
needs data to survive pod or VM replacement must currently manage encryption in
the application, expose the key or plaintext to the host, or depend on a
vendor-specific integration.

The host and CSI control path are outside the confidential guest's trust
boundary. They can select, replace, replay, resize, or corrupt a block device and
can alter unmeasured mount metadata. A persistent-storage design therefore
cannot treat a host request to decrypt a device as authorization.

## Goals

- Attach a CSI-provisioned raw block device to a confidential Kata guest without
  exposing its encryption key to the host.
- Authorize the volume identity, storage profile, and KBS key resource through
  measured policy before the guest mutates or opens the device.
- Create the encrypted filesystem on first use and reopen it after pod or VM
  replacement.
- Detect in-place ciphertext and integrity-metadata corruption.
- Keep ordinary direct-assigned volumes unchanged and fail closed for malformed,
  incomplete, or unsupported confidential-storage requests.
- Implement the host path in runtime-rs only. The deprecated Go runtime receives
  no new feature code.

## Non-goals for the first profile

- Shared or multi-writer volumes.
- Online or offline resize.
- Host-side filesystem mounting or key handling.
- Arbitrary encryption, integrity, or filesystem options supplied by a CSI
  driver.
- Rollback detection for replay of an entire older, internally consistent block
  device snapshot. That requires trusted monotonic state outside the volume and
  should be designed separately.

## Proposed contract

The first profile is intentionally fixed: LUKS2 encryption, journaled
dm-integrity, and ext4 on a direct-assigned raw block device.

The CSI integration supplies a typed, non-secret request containing:

- a versioned profile identifier;
- a stable volume ID; and
- a `kbs:///` resource URI identifying the key.

The same tuple must be present in measured init-data. The host request selects a
claim; it does not create one. Unknown fields, profiles, URI schemes, mount
options, filesystem creation flags, and resize requests are rejected before
device mutation.

## Component responsibilities

1. The CSI driver provisions a raw block volume and uses Kata's direct volume
   assignment interface. It never receives the encryption key.
2. runtime-rs parses and validates the complete typed request before hotplug and
   sends typed fields to the Kata Agent. It also owns attachment bookkeeping,
   statistics, and cleanup ordering.
3. The Kata Agent matches the request exactly against measured init-data before
   asking the Confidential Data Hub (CDH) to activate storage.
4. CDH retrieves the key from KBS after attestation, creates or reopens the fixed
   LUKS2/dm-integrity mapping, and mounts ext4 inside the guest.
5. Teardown unmounts the filesystem and closes the encrypted mapping before
   runtime-rs detaches the block device.

The key bytes remain inside the attested guest. Logs and status surfaces expose
only bounded structural state and non-secret identifiers.

## Lifecycle and failure behavior

On first attachment, CDH may format only after the measured claim and complete
request have been accepted. On later attachments, it must reopen the existing
header using the same key resource. A wrong key, mismatched volume identity,
unsupported format, integrity failure, or ambiguous device state fails without
falling back to plaintext or reformatting.

Deletion of a Kubernetes object does not imply destruction of the encryption
key. Key retirement and backing-volume deletion remain explicit storage-policy
operations. Snapshot and restore are allowed only as storage-system operations;
the first profile detects corruption but does not claim rollback detection.

## Security properties

The proposal provides:

- confidentiality from an untrusted host and storage backend;
- measured authorization of the volume/profile/key tuple;
- fail-closed downgrade and option handling;
- integrity checking for modified encrypted sectors and metadata; and
- guest-only key retrieval and activation.

It does not protect availability, hide access patterns or volume size, or detect
replay of a complete prior authenticated snapshot.

## Compatibility and rollout

The contract is additive. Existing direct volume metadata and non-confidential
volumes retain their current behavior. The initial profile should remain
experimental until runtime-rs, Agent, CDH, genpolicy, and a CSI integration have
the same lifecycle tests and the community agrees that the API is stable.

## Required tests

- First-use creation followed by write, detach, new-VM reopen, and read.
- Rejection before mutation for absent or mismatched measured claims, malformed
  metadata, unknown profiles/options, plaintext downgrade, and resize.
- Wrong-key and unavailable-KBS recovery without reformatting.
- Ciphertext and integrity-metadata corruption detection.
- Cleanup ordering: unmount, close mapping, then device detach.
- Confirmation that the host cannot observe the key or plaintext.
- Regression coverage for ordinary direct-assigned volumes.

## Alternatives considered

- **Application-managed encryption:** secure but not transparent and duplicates
  storage lifecycle logic in every workload.
- **Host-managed LUKS:** violates the confidential-computing trust boundary by
  exposing the key and plaintext to the host.
- **A new random key on every attach:** works for ephemeral storage but cannot
  reopen persistent data.
- **Free-form CSI driver options:** difficult to measure and prone to downgrade
  or incompatible combinations; a small versioned profile is safer.

## Questions for the community

1. Is direct volume assignment the right transport for the first persistent
   confidential-storage profile?
2. Is the runtime-rs/Agent/CDH ownership split above appropriate?
3. Should the first profile remain fixed and experimental, or use a different
   versioning mechanism?
4. Is explicit non-support for resize, sharing, and whole-volume replay
   detection acceptable for the initial scope?
5. Which project should own the cross-component conformance test and profile
   specification?

[issue-8515]: https://github.com/kata-containers/kata-containers/issues/8515
[implementation-pr]: https://github.com/kata-containers/kata-containers/pull/13637
[proposal-issue]: https://github.com/kata-containers/kata-containers/issues/13638
