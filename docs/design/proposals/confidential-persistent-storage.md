# Confidential Persistent Storage for Kata

## Authors

- Noel Jackson ([@noeljackson](https://github.com/noeljackson)) <n@noeljackson.com>

## Status

This is the current design for [kata-containers issue #13638](https://github.com/kata-containers/kata-containers/issues/13638). It supersedes the older draft in the issue body.

## Summary

This proposal lets a confidential Kata workload use persistent storage without exposing the volume key or plaintext to the node host. CSI supplies a raw block device. Inside the trusted guest, CDH prepares the device and Kata Agent mounts it. The application sees a normal directory, never the device or key.

This design does not require a new CSI driver and does not prevent a future CoCo CSI proxy or DAV-aware driver. The first version uses standard Kubernetes Block PVCs through `volumeDevices`, allowing a compatible block-mode CSI driver to remain unchanged. Future integrations may change how the device reaches Kata or provide a `volumeMount` interface, but they must feed the same policy-authorized guest activation flow while keeping the manifest contract, on-disk format, and existing encrypted volumes compatible.

The design has nine steps:

1. The trusted storage administrator prepares Trustee/KBS.
2. The workload declares the confidential volume, and `genpolicy` places the resulting Agent policy in init data.
3. CSI supplies the raw block device.
4. Kata matches the declared device to the real OCI block device, removes it from the application, and creates the typed request.
5. The Kata Agent security policy authorizes the request.
6. CDH retrieves and verifies the manifest and key.
7. CDH initializes or reopens the volume.
8. Kata Agent mounts the filesystem into the container.
9. Kata cleans up the volume.

## First version

The first version uses dynamically provisioned Block PVCs through standard Kubernetes `volumeDevices` with `shared_fs=none`. It supports one writer through `ReadWriteOncePod` (RWOP): one volume attached to one Kata sandbox. It does not support read-only-many because every guest would need to verify that it received the same immutable content; mounting each disk read-only is not enough. It uses fixed LUKS2, dm-integrity with HMAC, and ext4. Longhorn is the first tested CSI backend, but the guest contract is CSI-agnostic.

`volumeDevices` is the only first-version input path.

Static and pre-populated volumes, read-write-many, hostile-host writer fencing, snapshot freshness, online snapshots, resize, migration, and key rotation are not included.

## Project ownership

This end-to-end proposal lives in kata-containers because it defines how Kubernetes and CSI storage input crosses Kata's runtime, OCI, Agent-policy, and guest-mount boundaries. The Kata implementation is tracked in [issue #13638](https://github.com/kata-containers/kata-containers/issues/13638).

guest-components owns the trusted CDH volume lifecycle and `SecureVolumeService`. Trustee/KBS owns attestation and release policy. The downstream CoCo [Confidential Persistent Volumes document](https://github.com/noeljackson/guest-components/blob/confidential-persistent-storage/confidential-data-hub/docs/CONFIDENTIAL_PERSISTENT_VOLUMES.md) defines the platform-neutral CDH API, persistent volume format, and full security rationale.

## Why this approach

### Why a typed CDH API?

The existing CDH storage API accepts free-form storage and mount options. This proposal adds a smaller typed API so the caller cannot choose the key, encryption format, filesystem, or recovery behavior.

LUKS2 with dm-integrity is the first supported protection profile, not a requirement of the design. Other guest-side block-encryption formats can use the same flow when CDH implements them as reviewed typed profiles.

### Related work

- [Direct Block Device Assignment](../direct-blk-device-assignment.md) defines the existing DirectVolume handoff from CSI to Kata.
- The [CSI Direct Volume Driver](../../../src/tools/csi-kata-directvolume/README.md) implements that handoff.
- [Issue #12842](https://github.com/kata-containers/kata-containers/issues/12842) proposes a complementary conversion from `volumeMounts` to Block `volumeDevices` for ordinary non-confidential storage. It can provide another input path, but it does not replace trusted activation.
- [Issue #8515](https://github.com/kata-containers/kata-containers/issues/8515) describes the earlier request for guest-side storage decryption and mounting.
- [Kata Agent Policy](../../how-to/how-to-use-the-kata-agent-policy.md) and [`genpolicy`](../../../src/tools/genpolicy/README.md) define the measured authorization mechanism reused here.

### Why not a sidecar?

A sidecar would need the device, key, mount privileges, and mount propagation. CDH and Kata Agent can prepare the volume before the application starts without giving those privileges to a Pod container.

### Why not read-only-many yet?

A read-only mount prevents normal writes, but it does not prove that every guest received the same content. An untrusted host could give Guest A version 7 and Guest B version 6. Both disks could be read-only, but the guests would not be reading the same trustworthy content.

Confidential read-only-many therefore needs an immutable filesystem, a trusted content identity such as a dm-verity root hash in the manifest, and verification inside every guest before mounting. Some storage backends cannot attach one block volume to many nodes, so they may also need one identical verified clone per guest. That is a separate immutable-volume design.

### How is the LUKS2 header protected?

The LUKS2 header controls how the disk is decrypted, so its normal checksum is not enough against a malicious host. The first-version disk layout is:

```text
Disk offset   Size       Contents
0x00000000    16 MiB     Standard detached LUKS2 header
0x01000000    4 KiB      Authenticated state record A
0x01001000    4 KiB      Authenticated state record B
0x01002000    remaining  Encrypted ext4 data and dm-integrity tags
```

The first 16 MiB is a standard detached LUKS2 header generated by `cryptsetup`. CoCo defines the two state records that follow it. Each record is exactly 4 KiB; multibyte integers are big-endian and every reserved byte must be zero.

| Bytes | Field |
| --- | --- |
| 0-7 | Magic: `COCOPV\0\0` |
| 8-9 | Metadata-format version: `2` |
| 10 | State: `1` prepared, `2` initializing, or `3` ready |
| 11 | Reserved |
| 12-19 | Sequence: `1`, `2`, or `3`, matching the state |
| 20-51 | SHA-256 of the volume ID |
| 52-83 | SHA-256 of the volume version |
| 84-115 | SHA-256 of the exact manifest bytes |
| 116-119 | Protection-profile version: `1` |
| 120-123 | Reserved |
| 124-155 | HMAC-SHA256 of the complete detached LUKS2 header; zero only in the prepared state |
| 156-187 | HMAC-SHA256 of the complete state record |
| 188-4095 | Reserved |

Here, `volumeKey` is the 32-byte key released by Trustee/KBS:

```text
binding   = SHA256(volumeId) || SHA256(volumeVersion) || manifestDigest || BE32(profileVersion)
authKey   = HMAC-SHA256(volumeKey, "coco-cdh-persistent-luks2-auth-key-v2" || binding)
headerMac = HMAC-SHA256(authKey, "coco-cdh-persistent-luks2-header-v2" || binding || completeHeader)
recordMac = HMAC-SHA256(authKey, "coco-cdh-persistent-luks2-record-v2" || recordWithRecordMacZeroed)
```

CDH checks both state records and uses the authentic record with the newest valid sequence. It then copies the complete header into a guest-only temporary file, verifies `headerMac`, and gives that same verified copy to `cryptsetup`.

A changed or foreign header is rejected. Replaying an older complete valid volume is still possible and remains outside the first version. The downstream CoCo [Confidential Persistent Volumes document](https://github.com/noeljackson/guest-components/blob/confidential-persistent-storage/confidential-data-hub/docs/CONFIDENTIAL_PERSISTENT_VOLUMES.md) owns the implementation and recovery rationale.

## Trust boundary

| Component | Responsibility |
| --- | --- |
| Trusted storage administrator | Create the volume key and manifest and provision them in Trustee/KBS. |
| Attestation Service | Verify the guest evidence and confirm that the supplied init data matches the digest bound into it. |
| Trustee/KBS | Apply its resource policy to the attested init-data claims and release the manifest and key. |
| Workload owner | Declare the volume before the Kata Agent security policy and init data are generated. |
| CSI driver | Dynamically provision and attach the raw block device. |
| Kata runtime and shim | Match the real device, remove it from the application, and create the typed Agent request. |
| Kata Agent | Enforce the Kata Agent security policy, call CDH, and mount the returned device. |
| CDH | Retrieve and verify the manifest and key, authenticate the detached LUKS2 header, then initialize or reopen the volume. |

Three separate checks happen before the volume is used, and passing one does not replace the next:

1. The Attestation Service verifies the guest's TEE evidence and confirms that the supplied init data matches the digest bound into that evidence. This proves which guest and init data are running. It does not authorize release of a storage key.
2. Trustee/KBS applies its resource policy to the attested guest and init data. It releases the manifest and key only when they match the workload and storage declaration that the owner approved. Digest binding alone is not enough: the policy must require the approved init data.
3. Inside the guest, Kata Agent loads `policy.rego` from that init data and checks the actual storage operation. The device, manifest URI, access, ownership, and mount destination must match before Kata Agent calls CDH.

In short, attestation proves what is running, Trustee/KBS decides whether it may receive the storage secrets, and Kata Agent decides whether the requested mount may happen.

## Design walkthrough

### Step 1: The trusted storage administrator prepares Trustee/KBS

Before the Pod starts, the trusted storage administrator chooses the volume ID, version, size, and LUKS UUID, generates the key, and creates an immutable manifest. The checked-in Codewire documentation fixture uses 32 public test bytes of `0x5a`; those bytes are not deployment key material. Its resource payload is the following 456 UTF-8 bytes, serialized with compact separators in the displayed member order and with no BOM or trailing newline:

```json
{"schemaVersion":3,"volumeId":"be31063a-8ec8-46d5-aa17-75cda1729370","volumeVersion":"be31063a-8ec8-46d5-aa17-75cda1729370-v3","deviceSizeBytes":21474836480,"access":"readWrite","protection":{"type":"luks2-integrity-rw","profileVersion":1,"keyUri":"kbs:///default/codewire-workspace-luks/be31063a-8ec8-46d5-aa17-75cda1729370","keySha256":"60bf07c488aad18fda339df07e4fbc47b4f00be71711936f18d04d352ad01890","luksUuid":"f617a73e-b03d-4b58-b1ae-354c72276ea0"}}
```

The SHA-256 of those exact bytes is `de7120550b6e85aea6e1e436158e595a61048d46e121870e46595e9fdad0d3ec`, so the exact manifest URI is:

```text
kbs:///default/codewire-storage-manifests/sha256-de7120550b6e85aea6e1e436158e595a61048d46e121870e46595e9fdad0d3ec
```

The fixture and current CDH validation test are checked in on the downstream Guest Components branch at [`codewire-cpv-manifest-v3.fixture.json`](https://github.com/noeljackson/guest-components/blob/downstream/confidential-storage/confidential-data-hub/hub/test_files/codewire-cpv-manifest-v3.fixture.json).

For production, the administrator stores a random 32-byte key and its matching manifest in Trustee/KBS and configures their release policy. The manifest contains no key bytes. CDH must have a nonempty protected-resource prefix containing `default/codewire-workspace-luks`, every manifest `keyUri` must remain inside it, and Trustee must restrict both the exact manifest and key paths. The complete volume ID, volume version, manifest digest, and profile version form the persistent generation binding. Neither resource may be replaced after the manifest URI is included in the Kata Agent security policy. Neither CSI nor the node host receives the key.

### Step 2: The workload declares the confidential volume, and `genpolicy` places the resulting Agent policy in init data

For the primary path, the workload requests a Block PVC of the same size and declares how the filesystem will appear in the application:

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: workspace
spec:
  storageClassName: longhorn
  accessModes:
    - ReadWriteOncePod
  volumeMode: Block
  resources:
    requests:
      storage: 20Gi
---
apiVersion: v1
kind: Pod
metadata:
  name: workspace
  annotations:
    io.katacontainers.volume.confidential: >-
      {"workspace":{"devicePath":"/dev/confidential-workspace","manifestUri":"kbs:///default/codewire-storage-manifests/sha256-de7120550b6e85aea6e1e436158e595a61048d46e121870e46595e9fdad0d3ec","access":"readWrite","fsGroup":1000,"fsGroupChangePolicy":"OnRootMismatch","mounts":{"app":["/workspace"]}}}
spec:
  runtimeClassName: kata-coco
  securityContext:
    fsGroup: 1000
    fsGroupChangePolicy: OnRootMismatch
  containers:
    - name: app
      image: example.invalid/application@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
      volumeDevices:
        - name: workspace
          devicePath: /dev/confidential-workspace
  volumes:
    - name: workspace
      persistentVolumeClaim:
        claimName: workspace
```

The annotation contains the volume name, container-visible device path, content-addressed manifest URI, requested access, filesystem ownership, target container, and final mount destination. The final URI segment is `sha256-` followed by the SHA-256 of the exact manifest bytes. The annotation contains no key or host path.

[`genpolicy`](../../../src/tools/genpolicy/README.md) includes this declaration in `policy.rego`, the [Kata Agent security policy](../../how-to/how-to-use-the-kata-agent-policy.md), and places it in the Pod's `io.katacontainers.config.hypervisor.cc_init_data` annotation. Kata turns the annotation into a read-only init-data device for the guest and binds its digest into the TEE attestation evidence. Kata Agent reads and installs `policy.rego` before handling the storage request.

During attestation, the Attestation Service verifies that the init data matches the digest in the TEE evidence. The Trustee/KBS resource policy must authorize the storage declaration before KBS releases the manifest or key. If the host changes init data or the Agent policy, the measured digest changes and release is denied. If it changes the separate runtime declaration or device, Kata Agent rejects the resulting request against the unchanged policy.

### Step 3: CSI supplies the raw block device

For a new PVC, CSI dynamically creates and attaches a blank block device. On later Pods, CSI attaches the same device containing ciphertext. CSI does not retrieve the key, initialize encryption, format ext4, or mount the filesystem.

The first version passes the device through standard Kubernetes `volumeDevices`. Kata uses the device path declared in Step 2 and continues with Step 4.

### Step 4: Kata matches the declared device to the real OCI block device, removes it from the application, and creates the typed request

Kata matches the declared device path to exactly one real OCI block device, removes it and the shim-only annotation from the application, and creates the typed request. The request contains the actual guest device, manifest URI, `readWrite` access, guest mount point, and filesystem ownership.

This must be a recognized storage type, not only an optional protobuf field. An older Agent therefore rejects it instead of silently treating it as ordinary storage. Existing requests without the new type keep their current behavior. The Kata implementation is tracked in [issue #13638](https://github.com/kata-containers/kata-containers/issues/13638).

### Step 5: The Kata Agent security policy authorizes the request

Kata Agent checks the container, mount destination, manifest URI, access, filesystem ownership, and device transport. If anything differs from the Kata Agent security policy, it rejects the request before CDH retrieves a key or changes the device.

### Step 6: CDH retrieves and verifies the manifest and key

CDH uses the Attestation Agent to request the content-addressed manifest and versioned key from Trustee/KBS. Trustee/KBS releases them only after attestation and its resource policy succeed.

CDH verifies the manifest digest, supported schema version, key digest, access, device size, protection profile, and expected LUKS UUID. When reopening, it also verifies the on-disk LUKS identity. A failure returns before device mutation; a new blank disk receives the expected UUID during Step 7.

### Step 7: CDH initializes or reopens the volume

Kata Agent calls CDH through this typed `SecureVolumeService` contract:

```protobuf
enum VolumeAccess {
  VOLUME_ACCESS_UNSPECIFIED = 0;
  VOLUME_ACCESS_READ_ONLY = 1;
  VOLUME_ACCESS_READ_WRITE = 2;
}

message ActivateVolumeRequest {
  string device_id = 1;
  string manifest_uri = 2;
  VolumeAccess requested_access = 3;
}

message ActivateVolumeResponse {
  string activation_id = 1;
  string device_path = 2;
  VolumeAccess effective_access = 3;
}

message DeactivateVolumeRequest {
  string activation_id = 1;
}

message DeactivateVolumeResponse {}

service SecureVolumeService {
  rpc ActivateVolume(ActivateVolumeRequest) returns (ActivateVolumeResponse) {};
  rpc DeactivateVolume(DeactivateVolumeRequest) returns (DeactivateVolumeResponse) {};
}
```

For an all-zero device, CDH creates the fixed LUKS2 and dm-integrity layout from the manifest and formats ext4. For a non-empty device, CDH authenticates and reopens the expected volume. A failed reopen never causes formatting.

CDH verifies the authenticated detached header before `cryptsetup` can create a mapper. It returns an opaque activation ID, the plaintext mapper path, and the access it actually enforced. The mapper exists only inside the trusted guest. `VOLUME_ACCESS_READ_ONLY` is reserved for a later immutable profile; the first version accepts only `VOLUME_ACCESS_READ_WRITE`.

Treating an all-zero device as new means a hostile host can substitute another blank device. This can destroy continuity, but it does not expose the key or plaintext. Preventing that substitution requires a later trusted provisioning or freshness design.

The downstream CoCo [Confidential Persistent Volumes document](https://github.com/noeljackson/guest-components/blob/confidential-persistent-storage/confidential-data-hub/docs/CONFIDENTIAL_PERSISTENT_VOLUMES.md) owns the complete API, on-disk format, header authentication, and recovery rationale.

### Step 8: Kata Agent mounts the filesystem into the container

Kata Agent mounts ext4 from the mapper at an internal guest path, applies the authorized ownership, and bind-mounts it at the authorized application destination.

The application sees `/workspace` as a normal filesystem. It needs no mount entrypoint or storage sidecar and never receives the raw device, mapper, key, or mount privileges.

### Step 9: Kata cleans up the volume

After the last user releases the volume, Kata removes the container bind mount, unmounts ext4, asks CDH to deactivate the mapper, and releases the hotplugged device. Activation is sandbox-scoped and reference-counted.

## Failure behavior

- Missing or malformed input fails closed.
- The declaration must match exactly one OCI block device.
- Agent-policy or Trustee/KBS-policy denial happens before activation.
- A wrong manifest, key, size, profile, access, or initialized device is rejected.
- A modified or foreign LUKS2 header is rejected before `cryptsetup` opens it.
- A non-empty unknown device is never formatted.
- Read-only access and unsupported profiles are rejected.
- Errors never include keys, credentials, or plaintext.

## Security limits

The key and plaintext stay out of the node host. dm-integrity with HMAC detects ciphertext or tag forgery and corruption, but it does not detect replay of a previously valid ciphertext/tag pair to the same sector. CDH authenticates the complete detached LUKS2 header before using it. The trusted storage administrator and Trustee/KBS are part of the trusted control plane.

The node host still controls availability and can replay an older valid volume, substitute an all-zero device during first initialization, or attempt concurrent attachment. Same-sector ciphertext/tag replay, selective rollback, and mixed filesystem epochs are explicitly accepted first-version risks. RWOP reduces accidental concurrent writers but does not fence a hostile host. The first version does not claim rollback protection, freshness, or storage continuity.

Where rollback matters, the application must use application-level versioning or a trusted monotonic state service, with the expected state authenticated outside the hostile disk. Replay can cause ext4 to parse adversarial, replay-corrupted state inside the guest, so deployments must keep the guest kernel on a reviewed, patched baseline.

## Acceptance

The first version must demonstrate:

- dynamic Longhorn Block provisioning with `shared_fs=none`;
- the `volumeDevices` path producing the recognized Agent storage request authorized by the measured declaration;
- Trustee/KBS provisioning, attestation, and key release;
- refusal to release the manifest or key when the attested storage declaration does not match the Trustee/KBS resource policy;
- guest-only LUKS2, dm-integrity with HMAC, ext4, and plaintext mounting;
- rejection of a modified detached LUKS2 header before a mapper is created;
- reopen across Pod replacement and an authorized node reboot;
- failure for altered manifests, keys, access, destinations, and devices; and
- cleanup without exposing the raw device to the application.

CDH tests also use a loop-backed block device so the guest contract is tested independently of CSI.

## Later work

Static provisioning needs a trusted process that creates or imports the disk and publishes its matching manifest and key.

Confidential DirectVolume is later work. It must define a measured `volumeMount` declaration, make `genpolicy` authorize the exact manifest, access, ownership, destination, and transport, and add end-to-end tests showing that a valid request succeeds and every altered field is denied. Until then, `genpolicy` and runtime-rs reject confidential DirectVolume.

Read-only-many is a separate immutable profile, not a different LUKS header for this read-write design. The earlier section explains the required publication and verification model.

Read-write-many, resize, snapshot freshness, migration, and key rotation each need a separate design.

## Downstream validation work

Most of the substantial trusted-storage work is in CoCo guest-components. Kata supplies the runtime and Agent integration, while the extensions repository builds the exact components used for validation.

None of these branches has been submitted as a pull request. They were used downstream to validate the design concepts and provide implementation evidence that the flow can work. They are prototypes and may change before any upstream submission.

- [guest-components: `downstream/confidential-storage`](https://github.com/noeljackson/guest-components/tree/downstream/confidential-storage)
  - Adds the typed CDH API for activating and deactivating confidential volumes.
  - Retrieves and verifies the manifest and key through attested Trustee/KBS access.
  - Initializes a new volume or reopens an existing LUKS2, dm-integrity, and ext4 volume with fail-closed checks.
  - Owns activation state, recovery, and cleanup.
- [kata-containers: `downstream/confidential-storage`](https://github.com/noeljackson/kata-containers/tree/downstream/confidential-storage)
  - Accepts standard raw `volumeDevices` input and rejects confidential DirectVolume.
  - Removes the raw device from the application and creates the typed Agent request.
  - Authorizes the request, calls CDH, and mounts only the returned mapper.
  - Bind-mounts the filesystem into the application and handles cleanup.
- [extensions: `downstream/confidential-storage`](https://github.com/noeljackson/extensions/tree/downstream/confidential-storage)
  - Pins the exact guest-components, Kata, and Trustee sources used for validation.
  - Adds the LUKS2, dm-integrity, and ext4 tools required in the guest.
  - Builds and verifies the measured Kata guest and runtime extension.
  - Uses stock CSI without a patched Longhorn build.

Longhorn does not need to be modified for the primary `volumeDevices` path. The final downstream validation uses the stock Longhorn CSI deployment.

## FAQ

### Does the first version use static or dynamic provisioning?

Dynamic. CSI provisions and attaches a blank block volume for the PVC. The manifest and key are prepared beforehand, but the disk is not. Static or pre-populated disks require a trusted preparation and import process and are later work.

### Why does the first version use `volumeDevices`?

`volumeDevices` gives Kata the raw disk without mounting its filesystem on the host. A normal `volumeMount` is usually mounted on the host and shared into the guest. That does not work with `shared_fs=none`, and for confidential storage it would expose plaintext to the host.

### What does the application container receive?

A normal directory at its requested mount path. Kata removes the raw device from the application, CDH activates it, and Kata Agent mounts and bind-mounts the filesystem. The application never sees or mounts the device.

### Why is the confidential-volume declaration an annotation instead of CSI configuration?

CSI is responsible for provisioning and attaching the disk. The annotation declares the Kata operation that the workload owner authorizes, including the manifest, access, ownership, and destination. `genpolicy` copies those authorized values into the measured Kata Agent policy, so host-controlled CSI metadata cannot authorize the operation. The CSI driver does not read this annotation and does not need to be modified.

### Is the annotation an alternative to DirectVolume?

No. The annotation declares and authorizes the operation. DirectVolume is a CSI-to-Kata device handoff. A future confidential DirectVolume path would still need a measured declaration.

### Does DirectVolume work with every CSI driver?

No. Each CSI driver used with DirectVolume must implement the handoff, or a proxy must implement and delegate its CSI lifecycle. `csi-kata-directvolume` is its own storage driver and does not automatically wrap Longhorn, Ceph, or another CSI driver. The first version avoids that requirement by using standard Block PVCs and `volumeDevices`.

### How does this relate to non-confidential Kata with `shared_fs=none`?

Without CoCo, a standard `volumeDevice` reaches the application as a raw device; Kata does not currently turn it into a filesystem mount. The same device-to-mount mechanism can be generalized so an explicit Kata mount declaration makes Kata Agent format and mount the disk without CDH. The confidential path adds measured authorization and CDH activation before that mount. [Issue #12842](https://github.com/kata-containers/kata-containers/issues/12842) explores a generic front end for this behavior.

### Can static and read-only-many volumes be added later?

Yes. They can reuse the same block transport and activation API. They also need trusted disk preparation and an immutable, verifiable read-only profile, as described above.
