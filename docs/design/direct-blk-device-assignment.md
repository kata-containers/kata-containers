# Motivation
Today, there exist a few gaps between Container Storage Interface (CSI) and virtual machine (VM) based runtimes such as Kata Containers
that prevent them from working together smoothly.

First, it’s cumbersome to use a persistent volume (PV) with Kata Containers. Today, for a PV with Filesystem volume mode, Virtio-fs
is the only way to surface it inside a Kata Container guest VM. But often mounting the filesystem (FS) within the guest operating system (OS) is
desired due to performance benefits, availability of native FS features and security benefits over the Virtio-fs mechanism.

Second, it’s difficult if not impossible to resize a PV online with Kata Containers. While a PV can be expanded on the host OS,
the updated metadata needs to be propagated to the guest OS in order for the application container to use the expanded volume.
Currently, there is not a way to propagate the PV metadata from the host OS to the guest OS without restarting the Pod sandbox.

# Proposed Solution

Because of the OS boundary, these features cannot be implemented in the CSI node driver plugin running on the host OS
as is normally done in the runc container. Instead, they can be done by the Kata Containers agent inside the guest OS,
but it requires the CSI driver to pass the relevant information to the Kata Containers runtime.
An ideal long term solution would be to have the `kubelet` coordinating the communication between the CSI driver and
the container runtime, as described in [KEP-2857](https://github.com/kubernetes/enhancements/pull/2893/files).
However, as the KEP is still under review, we would like to propose a short/medium term solution to unblock our use case.

The proposed solution is built on top of a previous [proposal](https://github.com/egernst/kata-containers/blob/da-proposal/docs/design/direct-assign-volume.md)
described by Eric Ernst. The previous proposal has two gaps:

1. Writing a `csiPlugin.json` file to the volume root path introduced a security risk. A malicious user can gain unauthorized
access to a block device by writing their own `csiPlugin.json` to the above location through an ephemeral CSI plugin.

2. The proposal didn't describe how to establish a mapping between a volume and a kata sandbox, which is needed for
implementing CSI volume resize and volume stat collection APIs.

This document particularly focuses on how to address these two gaps.

## Assumptions and Limitations
1. The proposal assumes that a block device volume will only be used by one Pod on a node at a time, which we believe
is the most common pattern in Kata Containers use cases. It’s also unsafe to have the same block device attached to more than
one Kata pod. In the context of Kubernetes, the `PersistentVolumeClaim` (PVC) needs to have the `accessMode` as `ReadWriteOncePod`.
2. More advanced Kubernetes volume features such as, `fsGroup`, `fsGroupChangePolicy`, and `subPath` are not supported.

## End User Interface

1. The user specifies a PV as a direct-assigned volume. How a PV is specified as a direct-assigned volume is left for each CSI implementation to decide.
There are a few options for reference:
   1. A storage class parameter specifies whether it's a direct-assigned volume. This avoids any lookups of PVC
   or Pod information from the CSI plugin (as external provisioner takes care of these). However, all PVs in the storage class with the parameter set
   will have host mounts skipped.
   2. Use a PVC annotation. This approach requires the CSI plugins have `--extra-create-metadata` [set](https://kubernetes-csi.github.io/docs/external-provisioner.html#persistentvolumeclaim-and-persistentvolume-parameters)
   to be able to perform a lookup of the PVC annotations from the API server. Pro: API server lookup of annotations only required during creation of PV.
   Con: The CSI plugin will always skip host mounting of the PV.
   3. The CSI plugin can also lookup pod `runtimeclass` during `NodePublish`. This approach can be found in the [ALIBABA CSI plugin](https://github.com/kubernetes-sigs/alibaba-cloud-csi-driver/blob/master/pkg/disk/nodeserver.go#L248).
2. The CSI node driver delegates the direct assigned volume to the Kata Containers runtime. The CSI node driver APIs need to
   be modified to pass the volume mount information and collect volume information to/from the Kata Containers runtime by invoking `kata-runtime` command line commands.
   * **`NodePublishVolume`** -- It invokes `kata-ctl direct-volume add --volume-path [volumePath] --mount-info [mountInfo]`
   to propagate the volume mount information to the Kata Containers runtime for it to carry out the filesystem mount operation.
   The `volumePath` is the [target_path](https://github.com/container-storage-interface/spec/blob/master/csi.proto#L1364) in the CSI `NodePublishVolumeRequest`.
   The `mountInfo` is a serialized JSON string.
   * **`NodeGetVolumeStats`** -- It invokes `kata-ctl direct-volume stats --volume-path [volumePath]` to retrieve the filesystem stats of direct-assigned volume.
   * **`NodeExpandVolume`** -- It invokes `kata-ctl direct-volume resize --volume-path [volumePath] --size [size]` to send a resize request to Kata Containers to
   resize the direct-assigned volume.
   * **`NodeStageVolume/NodeUnStageVolume`** -- It invokes `kata-ctl direct-volume remove --volume-path [volumePath]` to remove the persisted metadata of a direct-assigned volume.

The `mountInfo` object consumed by runtime-rs is defined as follows:
```Rust
pub struct DirectVolumeMountInfo {
    pub volume_type: String,
    pub device: String,
    pub fs_type: String,
    pub metadata: HashMap<String, String>,
    pub options: Vec<String>,
    pub confidential_storage: Option<ConfidentialStorage>,
}

pub struct ConfidentialStorage {
    pub profile: String,
    pub volume_id: String,
    pub key_uri: String,
}
```
Notes: given that the `mountInfo` is persisted to disk by the Kata runtime, it shouldn't contain any secrets (such as an SMB mount password).

Confidential persistent storage uses the typed `confidential-storage` object,
not driver options or vendor-specific metadata. Its initial fixed profile is:

```json
{
  "volume-type": "directvol",
  "device": "/dev/example",
  "fstype": "confidential-storage",
  "confidential-storage": {
    "profile": "luks2-integrity-ext4",
    "volume-id": "tenant/workload/volume",
    "key-uri": "kbs:///tenant/storage/key"
  }
}
```

runtime-rs strictly parses and validates this complete object before attaching
the device, then transports it to the guest as a dedicated Agent protocol
message. The Agent authorizes it
only when the exact profile, volume ID, and key URI tuple appears in the
versioned `confidential_storage` registry in measured init-data. That registry
is a JSON string under the init-data `data` table:

```json
{
  "version": 1,
  "volumes": [{
    "profile": "luks2-integrity-ext4",
    "volumeId": "tenant/workload/volume",
    "keyUri": "kbs:///tenant/storage/key"
  }]
}
```

The outer `confidential-storage` fstype is a fail-closed discriminator, not the
on-disk filesystem. An older runtime or Agent that does not understand the
typed contract will fail instead of silently mounting a raw ext4 volume. The fixed
profile asks CDH for persistent, journaled LUKS2 integrity and ext4 activation;
it cannot be combined with filesystem-creation, ephemeral-encryption, sharing,
or mount driver options. Its resize contract is deliberately unsupported and
must fail without mutation until a separate profile defines safe growth.

The KBS URI and stable volume ID are non-secret identifiers. Plaintext key
material is released to CDH only inside the attested guest and is never included
in `mountInfo`, Agent protocol fields, or driver options.

## Implementation Details

### Kata runtime
Instead of the CSI node driver writing the mount info into a `csiPlugin.json` file under the volume root,
as described in the original proposal, here we propose that the CSI node driver passes the mount information to
the Kata Containers runtime through a new `kata-runtime` commandline command. The `kata-runtime` then writes the mount
information to a `mountInfo.json` file in a predefined location (`/run/kata-containers/shared/direct-volumes/[volume_path]/`).

When the Kata Containers runtime starts a container, it verifies whether a volume mount is a direct-assigned volume by checking
whether there is a `mountInfo` file under the computed Kata `direct-volumes` directory. If it is, the runtime parses the `mountInfo` file,
updates the mount spec with the data in `mountInfo`. The updated mount spec is then passed to the Kata agent in the guest VM together
with other mounts. The Kata Containers runtime also creates a file named by the sandbox id under the `direct-volumes/[volume_path]/`
directory. The reason for adding a sandbox id file is to establish a mapping between the volume and the sandbox using it.
Later, when the Kata Containers runtime handles the `get-stats` and `resize` commands, it uses the sandbox id to identify
the endpoint of the corresponding `containerd-shim-kata-v2`.

### containerd-shim-kata-v2 changes
`containerd-shim-kata-v2` provides an API for sandbox management through a Unix domain socket. Two new handlers are proposed: `/direct-volume/stats` and `/direct-volume/resize`:

Example:

```bash
$ curl --unix-socket "$shim_socket_path" -I -X GET 'http://localhost/direct-volume/stats/[urlSafeVolumePath]'
$ curl --unix-socket "$shim_socket_path" -I -X POST 'http://localhost/direct-volume/resize' -d '{ "volumePath"": [volumePath], "Size": "123123" }'
```

The shim then forwards the corresponding request to the `kata-agent` to carry out the operations inside the guest VM. For `resize` operation,
the Kata runtime also needs to notify the hypervisor to resize the block device (e.g. call `block_resize` in QEMU).

### Kata agent changes

The mount spec of a direct-assigned volume is passed to `kata-agent` through the existing `Storage` GRPC object.
Two new APIs and three new GRPC objects are added to GRPC protocol between the shim and agent for resizing and getting volume stats:
```protobuf

rpc GetVolumeStats(VolumeStatsRequest) returns (VolumeStatsResponse);
rpc ResizeVolume(ResizeVolumeRequest) returns (google.protobuf.Empty);

message VolumeStatsRequest {
// The volume path on the guest outside the container
    string volume_guest_path = 1;
}

message ResizeVolumeRequest {
// Full VM guest path of the volume (outside the container)
    string volume_guest_path = 1;
    uint64 size = 2;
}

// This should be kept in sync with CSI NodeGetVolumeStatsResponse (https://github.com/container-storage-interface/spec/blob/v1.5.0/csi.proto)
message VolumeStatsResponse {
   // This field is OPTIONAL.
   repeated VolumeUsage usage = 1;
   // Information about the current condition of the volume.
   // This field is OPTIONAL.
   // This field MUST be specified if the VOLUME_CONDITION node
   // capability is supported.
   VolumeCondition volume_condition = 2;
}
message VolumeUsage {
   enum Unit {
      UNKNOWN = 0;
      BYTES = 1;
      INODES = 2;
   }
   // The available capacity in specified Unit. This field is OPTIONAL.
   // The value of this field MUST NOT be negative.
   uint64 available = 1;

   // The total capacity in specified Unit. This field is REQUIRED.
   // The value of this field MUST NOT be negative.
   uint64 total = 2;

   // The used capacity in specified Unit. This field is OPTIONAL.
   // The value of this field MUST NOT be negative.
   uint64 used = 3;

   // Units by which values are measured. This field is REQUIRED.
   Unit unit = 4;
}

// VolumeCondition represents the current condition of a volume.
message VolumeCondition {

   // Normal volumes are available for use and operating optimally.
   // An abnormal volume does not meet these criteria.
   // This field is REQUIRED.
   bool abnormal = 1;

   // The message describing the condition of the volume.
   // This field is REQUIRED.
   string message = 2;
}

```

### Step by step walk-through

Given the following definition:
```YAML
---
apiVersion: v1
kind: Pod
metadata:
  name: app
spec:
  runtime-class: kata-qemu
  containers:
  - name: app
    image: centos
    command: ["/bin/sh"]
    args: ["-c", "while true; do echo $(date -u) >> /data/out.txt; sleep 5; done"]
    volumeMounts:
    - name: persistent-storage
      mountPath: /data
  volumes:
  - name: persistent-storage
    persistentVolumeClaim:
      claimName: ebs-claim
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  annotations:
    skip-hostmount: "true"
  name: ebs-claim
spec:
  accessModes:
    - ReadWriteOncePod
  volumeMode: Filesystem
  storageClassName: ebs-sc
  resources:
    requests:
      storage: 4Gi
---
kind: StorageClass
apiVersion: storage.k8s.io/v1
metadata:
  name: ebs-sc
provisioner: ebs.csi.aws.com
volumeBindingMode: WaitForFirstConsumer
parameters:
  csi.storage.k8s.io/fstype: ext4

```
Let’s assume that changes have been made in the `aws-ebs-csi-driver` node driver.

**Node publish volume**
1. In the node CSI driver, the `NodePublishVolume` API invokes: `kata-runtime direct-volume add --volume-path "/kubelet/a/b/c/d/sdf" --mount-info "{\"Device\": \"/dev/sdf\", \"fstype\": \"ext4\"}"`.
2. The `Kata-runtime` writes the mount-info JSON to a file called `mountInfo.json` under `/run/kata-containers/shared/direct-volumes/kubelet/a/b/c/d/sdf`.

**Node `unstage` volume**
1. In the node CSI driver, the `NodeUnstageVolume` API invokes: `kata-runtime direct-volume remove --volume-path "/kubelet/a/b/c/d/sdf"`.
2. Kata-runtime deletes the directory `/run/kata-containers/shared/direct-volumes/kubelet/a/b/c/d/sdf`.

**Use the volume in sandbox**
1. Upon the request to start a container, the `containerd-shim-kata-v2` examines the container spec,
and iterates through the mounts. For each mount, if there is a `mountInfo.json` file under `/run/kata-containers/shared/direct-volumes/[mount source path]`,
it generates a `storage` GRPC object after overwriting the mount spec with the information in `mountInfo.json`.
2. The shim sends the storage objects to kata-agent through TTRPC.
3. The shim writes a file with the sandbox id as the name under `/run/kata-containers/shared/direct-volumes/[mount source path]`.
4. The kata-agent mounts the storage objects for the container.

**Node expand volume**
1. In the node CSI driver, the `NodeExpandVolume` API invokes: `kata-runtime direct-volume resize –-volume-path "/kubelet/a/b/c/d/sdf" –-size 8Gi`.
2. The Kata runtime checks whether there is a sandbox id file under the directory `/run/kata-containers/shared/direct-volumes/kubelet/a/b/c/d/sdf`.
3. The Kata runtime identifies the shim instance through the sandbox id, and sends a GRPC request to resize the volume.
4. The shim handles the request, asks the hypervisor to resize the block device and sends a GRPC request to Kata agent to resize the filesystem.
5. Kata agent receives the request and resizes the filesystem.

**Node get volume stats**
1. In the node CSI driver, the `NodeGetVolumeStats` API invokes: `kata-runtime direct-volume stats –-volume-path "/kubelet/a/b/c/d/sdf"`.
2. The Kata runtime checks whether there is a sandbox id file under the directory `/run/kata-containers/shared/direct-volumes/kubelet/a/b/c/d/sdf`.
3. The Kata runtime identifies the shim instance through the sandbox id, and sends a GRPC request to get the volume stats.
4. The shim handles the request and forwards it to the Kata agent.
5. Kata agent receives the request and returns the filesystem stats.
