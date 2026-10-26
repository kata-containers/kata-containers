# Confidential Operations (cc-op) Design

## Overview

Confidential Container Operations (`cc-op`) provides authenticated and encrypted support for
operations in Confidential Containers (CoCo) environments. This document describes a proposed design
for `exec` and `cp` protection.

Without `cc-op`, these operations route unprotected data through an untrusted host control plane.
`cc-op` closes that gap by establishing an end-to-end cryptographic channel between the workload
owner and the `kata-agent` running inside the confidential guest VM, so neither the host OS, the
container runtime, nor the Kubernetes control plane can observe or tamper with operational traffic.

---

## Motivation

### Current State

Kata Containers sets up the stdin, stdout, and stderr streams for containers and processes running
inside the guest VM. On the host, IO channels are created and associated with running
containers and processes. Operations such as `kubectl exec` and `kubectl cp` rely on these
host-side mechanisms to exchange data between users and workloads.

The standard Kubernetes flows work as follows:

- **exec / cp:** The user runs `kubectl exec` or `kubectl cp`. The request arrives at the
  Kubernetes API server, which proxies it to the container runtime (e.g., containerd) via CRI. The
  container runtime forwards it to the Kata runtime (`containerd-shim-kata-v2`), which calls the
  kata-agent over a ttrpc/vsock channel inside the guest VM. The kata-agent launches the process
  and streams stdin/stdout/stderr back up the same path. For `cp`, `kubectl` internally uses `tar`
  piped through an exec stream.

Each component in that chain sees the unencrypted byte stream.

### Requirements

To support the CoCo threat model, `exec` and `cp` interfaces must:

1. Protect the confidentiality and integrity of `kubectl exec` commands and arguments.
2. Protect the confidentiality and integrity of stdin, stdout, and stderr streams.
3. Authenticate operational requests to the confidential guest.
4. Preserve existing Kubernetes user workflows as much as possible.
5. Minimize changes to the existing Kata Containers control-plane architecture.

---

## Threat Model

### Trusted Components

| Component | Rationale |
|---|---|
| Confidential guest VM (TEE) | Protected by hardware memory encryption and integrity guarantees. |
| `kata-agent` | Runs inside the TEE; its image is measured at boot. |
| Trustee / Key Broker Service (KBS) | Provisions secrets to attested guests; assumed honest. |
| User / Sender / Workload owner | Holds the sender's private key used in a [Hybrid Public Key Encryption (HPKE)](https://www.rfc-editor.org/info/rfc9180/) authenticated key exchange with the `kata-agent`. The user's public key is embedded in the initdata. |

### Untrusted Components

| Component | Threat |
|---|---|
| Kubernetes control plane | Can read or modify traffic that passes through it. |
| Cluster administrators | Have root access to the host. |
| Host OS | Owns virtual devices and vsock transport. |
| Container runtime (containerd, kata-shim) | Sits in the data path between user and guest. |
| Network between host components | Subject to interception or injection. |

### Security Goals

1. **Command confidentiality** — exec arguments and copy payloads are not visible to the host.
2. **Command integrity** — the `kata-agent` rejects tampered commands; replay protection for exec payloads is planned (see Future Work).
3. **Stream confidentiality** — stdin/stdout/stderr are encrypted in transit through the host.
4. **Stream integrity** — authenticated encryption detects any modification to stream data.
5. **User authentication** — only a holder of the authorized private key can issue protected operations.

### Non-Goals

- Protecting workloads from a compromised guest kernel or a malicious container workload.
- Auditing, authorization policy, or multi-tenant key management.
- End-to-end protection of the Kubernetes API server itself (only the data path is protected).
- Protection for `attach` and `logs` (planned for a follow-on release).

---

## Architecture

### Functionality Matrix

`cp` shares the exec mechanism on the client side; there are no additional agent-side changes for `cp`.

| Operation | Security Properties | User Keys | Trustee Keys | Stream Redirect | Notes |
|---|---|---|---|---|---|
| `exec` / `cp` | Authentication, Confidentiality, Integrity | Owner public/private key pair | Agent public/private key pair | Yes (stdin/stdout/stderr) | Authenticated encryption; replay resistant on IO Streams |

### Operational Diagram
```
┌──────────────────────────────────────────────────────────────────┐
│  User workstation                                                │
│  ┌─────────────────┐   ┌──────────────────────────────────────┐  │
│  │  kubectl / tool │──▶│  cc-op client library                │  │
│  └─────────────────┘   │  - HPKE encrypt command              │  │
│                        │  - Derive stream keys                │  │
│                        │  - Encrypt/decrypt IO streams        │  │
│                        └──────────────────────────────────────┘  │
└───────────────────────────────────┬──────────────────────────────┘
                                    │ encrypted payload + stream
                                    ▼
┌──────────────────────────────────────────────────────────────────┐
│  Kubernetes / host control plane  (UNTRUSTED)                    │
│  API server → containerd → kata-shim → vsock                     │
│  (carries ciphertext; cannot observe or modify plaintext)        │
└───────────────────────────────────┬──────────────────────────────┘
                                    │ encrypted bytes over vsock
                                    ▼
┌──────────────────────────────────────────────────────────────────┐
│  Confidential guest VM  (TRUSTED)                                │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  kata-agent  (cc-op feature enabled)                        │ │
│  │  - Holds agent P-256 private key (provisioned at boot)      │ │
│  │  - Verifies HPKE envelope (user auth cert)                  │ │
│  │  - Decrypts command arguments                               │ │
│  │  - Derives per-session stream keys                          │ │
│  │  - Encrypts outbound streams (stdout/stderr)                │ │
│  │  - Decrypts inbound stream (stdin)                          │ │
│  └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘

Workload owner ── cc-op client ──▶ generates payload ──▶ kubectl ──▶ host
                                ◀── encrypts streams ◀──────────────────
```

### Deployment Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Kubernetes Control Plane                                                │
│  API-Server ── Scheduler ── Controller ── etcd                          │
└──────────────────────────┬──────────────────────────────────────────────┘
                           │ CRI
┌──────────────────────────▼──────────────────────────────────────────────┐
│ Worker Node (Host System)  [UNTRUSTED]                                  │
│  kubectl ── Kubelet ── containerd ── kata-shim ── vsock                 │
└──────────────────────────┬──────────────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────────────┐
│ Confidential VM  [TRUSTED]                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │ kata-agent (cc-op enabled)                                          ││
│  └─────────────────────────────────────────────────────────────────────┘│
│  Pod / workload container                                               │
└──────────────────────────┬──────────────────────────────────────────────┘
                           │ attestation + key provisioning
┌──────────────────────────▼──────────────────────────────────────────────┐
│ Trustee / KBS                                                           │
│  - attests guest TEE                                                    │
│  - provisions agent P-256 key pair                                      │
└─────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility |
|---|---|
| **ccop client** | Encrypts the exec command using HPKE; derives stream keys from the HPKE context; encrypts stdin and decrypts stdout/stderr. |
| **Kubernetes / host** | Opaque transport; forwards ciphertext without modification. |
| **kata-agent** | Verifies and decrypts the HPKE envelope; extracts plaintext arguments; instantiates per-process stream processors; encrypts outbound and decrypts inbound IO. |
| **Trustee / KBS** | Provisions the agent's P-256 private key after remote attestation of the guest VM (and a symmetric key reserved for future `attach`/`logs` support). |

---

## Protocol

### High-Level Flow

```
User                          kata-agent               Trustee/KBS
 │                                 │                        │
 │  1. Obtain agent's public key   │                        │
 │     certificate from Trustee    │                        │
 │     (ccop fetch)                │                        │
 │                                 │  2. Attest guest ───▶  │
 │                                 │◀── provision priv key ─│
 │                                 │                        │
 │  3. HPKE-encrypt command        │                        │
 │     + base64-encode payload     │                        │
 │                                 │                        │
 │  4. ExecProcess(--payload=…) ──▶│                        │
 │                                 │  verify + decrypt      │
 │                                 │  extract args          │
 │                                 │  derive stream keys    │
 │                                 │                        │
 │◀── encrypted stdout/stderr ─────│                        │
 │──  encrypted stdin ────────────▶│                        │
```

### Initialization

At startup, `kata-agent` reads the initdata. This file contains: 

- `ccop.toml` which specifies an `auth_cert` — an X.509 PEM certificate holding the user's P-256 ECDSA public key (the verifying key). This is the certificate generated by the workload owner (see [User Workflow](../../how-to/how-to-use-ccop.md)) and embedded in the initdata at deployment time.
- pod's `name` and `ns` (namespace). The credential generator plugin [`credgen`](#target-credgen) requires them to verify that a credential request comes from the pod that owns them. They must match the pod's `metadata.name` and namespace.

### Agent Policy Interaction

When `cc-op` is active (`is_ccop_set() == true`), the `exec_process` ttrpc handler **skips** the `is_allowed()` policy check, so the `ExecProcessRequest` policy setting has no effect. Authentication is instead handled by HPKE: only the holder of the private key corresponding to `auth_cert` can produce a payload that passes verification.

The `ExecProcessRequest` setting is relevant only when the agent image is built with the `cc-op` feature but `cc-op` is not active (no `ccop.toml` in the initdata). In that case the policy check runs as in a baseline agent.

### Protocol Summary

The cc-op client wraps the real command in an HPKE Auth-mode envelope (KEM: P-256,
KDF: HKDF-SHA-256, AEAD: AES-128-GCM) using the agent's public key. The agent's identity as the
intended recipient, and the user's identity as the authenticated sender, are both cryptographically
bound to the ciphertext. On receipt, the agent decrypts the envelope, recovers the real arguments,
and derives per-session stream keys from the same HPKE context. Subsequent stdin/stdout/stderr
traffic for that exec session is encrypted and authenticated with those keys.

Stream data is protected using AES-256-GCM with per-record framing in both directions. Each
record carries a header (length + sequence number) that is fed as Additional Authenticated Data
(AAD) to AES-GCM, so any tampering with the record boundaries or sequence number is detected
by the authentication tag. A strictly monotonic sequence counter on both client and agent prevents
replay and reordering of individual stream records. This applies equally to non-TTY exec
(stdin/stdout/stderr as separate pipes) and TTY exec (multiplexed over a single PTY channel).

`cp` is implemented entirely on the client side by routing file data through `cc-op exec`. No
agent-side changes are required beyond what is already in place for exec.

### Implementation Notes

The cc-op feature spans four repositories:

- **`guest-components` / `cc-op` crate** — core cryptographic primitives: HPKE decryption, stream
  key derivation, and the `StreamProcessor` abstraction with its concrete exec implementations.
- **`kata-agent-cc-op` wrapper crate** — a thin shim that conditionally exposes the `cc-op` crate when the `cc-op` Cargo feature is enabled.
- **`kata-agent` (this repository)** — integrates the wrapper at startup, in the exec handler, and in the read/write stream handlers. The feature is gated by the `cc-op` Cargo feature flag.
- <a id="target-credgen"></a>**Trustee / KBS / CredGen Plugin** — delivers the agent's P-256 private key after remote attestation. In the same provisioning step, the agent also retrieves a symmetric key that is held in memory and is not used by `exec`; it is reserved for the follow-on `attach` and `logs` protection (see [Future Work](#future-work)).

---

## Key Management

### Agent Receiver Key

The `kata-agent` holds a P-256 private key that acts as the HPKE receiver key. This key is
provisioned to the guest by Trustee after successful remote attestation, stored only in process
memory, and never written to disk or exposed over any interface.

### User Authentication Certificate

The workload owner's X.509 certificate (containing their P-256 public key) is delivered to the
guest via the initdata mechanism at VM boot. Because HPKE Auth mode binds the sender's public key
to the ciphertext, any message that passes decryption is guaranteed to have been produced by the
holder of the corresponding private key.

### Key Lifetime

| Key | Lifetime |
|---|---|
| Agent receiver private key | Lifetime of the guest VM; provisioned once by Trustee. |
| Symmetric key (reserved) | Lifetime of the guest VM; provisioned once by Trustee; not used by `exec`. |
| User verifying key | Embedded in initdata; fixed for the VM lifetime. |
| Per-exec stream keys | Lifetime of a single exec session; derived fresh from each HPKE invocation. |

Key rotation is not currently implemented and is listed as future work.

---

## Compatibility

### Backward Compatibility

`cc-op` protection is activated at runtime by the presence of `ccop.toml` in the initdata, not
solely by the compiled-in feature flag. A `kata-agent` image built with the `cc-op` feature behaves
identically to a baseline agent when `ccop.toml` is absent from the initdata. A single agent image
can therefore be deployed across both `cc-op`-enabled and conventional pods; the difference is
entirely in the initdata delivered to each VM at boot.

### cc-op Client Tool

The `ccop` user-facing tool (which wraps `kubectl` under the hood, handling payload construction,
HPKE encryption, and stream en/decryption) is released in a separate repository. It is not part of
the `kata-containers` or `guest-components` repositories.

### Existing `kubectl` Workflows

Users who do not use the cc-op client tool continue to use `kubectl exec` and `kubectl cp` without
modification against pods that do not have `ccop.toml` in their initdata. For pods that do have
cc-op enabled, direct `kubectl exec` calls will be rejected by the agent because no encrypted
payload is present. Cluster operators are expected to configure RBAC or admission policies to
prevent non-cc-op clients from reaching cc-op–enabled pods.

---

## Security Considerations

### Replay Protection

HPKE is a one-shot mechanism; each invocation produces an independent encapsulated key and
ciphertext. Replay protection currently covers the IO streams: per-session stream frames are
protected against replay and reordering within a session by a strictly monotonic counter embedded
in each frame. Replay protection for the exec payload itself is not yet enforced. Adversaries
can resubmit captured exec payloads and run the same commands again. However, they cannot read
the output or change the commands. The `timestamp` field in the payload is included as
additional authenticated data (AAD) so that the agent can enforce a freshness window, but that
enforcement is listed as future work.

### Key Compromise

The agent's receiver key has the lifetime of the guest VM. Compromising it exposes all past
captured traffic because the IO stream keys are derived from the HPKE context. HPKE provides no
forward secrecy against the receiver key. The key is stored only in process memory and never
written to disk, and Trustee provisioning ensures it is only delivered to an attested guest.

### Passfd IO Mode

Stream encryption is not supported in passfd IO mode. In this mode, the container IO bypasses the WriteStdin / ReadStdout / ReadStderr ttrpc RPCs entirely. Data flows through dedicated vsock streams via raw `tokio::io::copy` tasks that do not consult the cc-op `stream_processor`. As a result, stdin, stdout, and stderr are not encrypted or authenticated in this mode. Pods using passfd IO therefore fall outside the cc-op confidentiality and integrity guarantees for stream data, regardless of whether `ccop.toml` is present in the initdata. See Future Work for the planned fail-closed behavior.

### Denial of Service

A host adversary can drop or corrupt vsock traffic, causing an exec or stream to fail. This is an
availability attack rather than a confidentiality or integrity attack, and is accepted as a
non-goal.

### Host Tampering

All exec stream data — both non-TTY and TTY — is authenticated end-to-end using AES-256-GCM.
The agent verifies the authentication tag on every record before delivering any plaintext to the
process. A host adversary can still drop or delay vsock traffic (causing the session to fail), but
cannot silently modify or inject stream content without detection.

---

## Future Work

- **Replay protection (timestamp enforcement)** — reject exec payloads with a timestamp outside an
  acceptable freshness window.
- **Key rotation** — support refreshing the agent receiver key without restarting the VM.
- **attach and logs protection** — extend cc-op to cover `attach` and `logs` with keys derived
  from the Trustee-provisioned symmetric key.
- **Fail closed on passfd** — if passfd IO mode and cc-op are both enabled, the pod or workload
  fails to start instead of silently running without stream protection.

---

*For the end-to-end user workflow — key generation, initdata construction, workload deployment, and
running cc-op operations — see [How to Use cc-op](../../how-to/how-to-use-ccop.md).*
