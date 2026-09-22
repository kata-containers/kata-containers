# How to run a rootless VMM

Kata Containers can run the virtual machine monitor (VMM) as an unprivileged
user instead of `root`. Each sandbox gets a throwaway user and group of its own
(`kata-123`), the VMM is started as that user, and the VMM's runtime files live
under `/run/user/[uid]/` where only that user can reach them.

!!! info "What this covers, and what it does not"
    Only the VMM process drops privileges. The Kata shim and `virtiofsd` still
    run as `root` on the host, and nothing changes inside the guest. See
    [Limitations](#limitations) for the features this mode gives up.

*[VMM]: Virtual Machine Monitor
*[TEE]: Trusted Execution Environment
*[QGS]: Quote Generation Service
*[UDS]: Unix Domain Socket

## Supported hypervisors

Rootless VMM is supported by the **runtime-rs** shims. That includes
`qemu-runtime-rs` and its variants (`qemu-snp-runtime-rs`, `qemu-tdx-runtime-rs`,
`qemu-se-runtime-rs`, `qemu-coco-dev-runtime-rs`, and the `qemu-nvidia-*` ones),
as well as `clh-runtime-rs`. They validate the host access an unprivileged VMM
needs *before* dropping privileges.

The QEMU runtime-rs variants are the only ones that support TEE devices. Cloud
Hypervisor runtime-rs supports rootless for non-confidential workloads.

The Go runtime's QEMU shims accept a `rootless` setting of their own, but they
only ever handled `/dev/kvm`, so they cannot run a confidential rootless sandbox.
Firecracker has no `rootless` setting; use its jailer instead.

## The host device access contract

A per-sandbox VMM user is created moments before the VMM starts. It owns
nothing, and it is a member of no group the host administrator chose. So for
every host device the VMM opens, the node has to grant access to a user that
does not exist yet.

Kata's rule for that, which the shim checks before it drops privileges, is:

1. The path must be a **character device**.
2. It must grant **read *and* write** — QEMU opens all of these `O_RDWR`, so
   read-only permission bits are not enough.
3. That access must come either from the **other** permission bits, or from a
   **group whose GID is not 0**. The shim then adds that GID to the VMM user's
   supplementary groups.

A group-`root` device is refused rather than used: putting the VMM user into
group 0 would hand it every other root-group-owned resource on the node.

!!! failure "A device that does not satisfy the contract fails the sandbox"
    The shim refuses to create the sandbox and names the device, its mode and
    the access it needed. It deliberately does not relax permissions on the fly
    — device ownership is host policy, so it is the deployment's to set.

### The devices

Only the devices the VMM opens **by path** need any of this:

| Device | Group | Needed by |
| --- | --- | --- |
| `/dev/kvm` | `kvm` | every VMM |
| `/dev/sev` | `kata-sev` | AMD SEV-SNP guest launch |
| `/dev/uv` | `kata-se` | IBM Secure Execution attestation |

`/dev/kvm` usually already satisfies the contract, because most distributions
ship it as `root:kvm 0660`. The TEE devices never do: they come up as
`root:root 0600`.

Each TEE device gets a group of its own rather than joining `kvm`, on purpose. A
rootless VMM receives the groups for the devices *its own shim* needs, so a
plain `qemu-runtime-rs` sandbox on an SNP host does not reach `/dev/sev` merely
because it needs `/dev/kvm`. That matters more than it looks: `/dev/sev` is the
platform interface to the AMD PSP rather than a per-VM one, and parts of its
surface affect every SNP guest on the host.

`/dev/kvm`, in contrast, keeps the conventional `kvm` group. A device node has
exactly one owning group, so moving it to a group of Kata's own would take
`/dev/kvm` away from whatever else on the node runs virtual machines.

!!! tip "The groups are created empty"
    `kata-sev` and `kata-se` are system groups with no members. Creating them
    and giving them group access grants nothing to any existing user; only the
    per-sandbox VMM user is ever added, and only for the duration of its
    sandbox.

!!! info "Why `/dev/vhost-vsock` and `/dev/vhost-net` are not in the list"
    The shim opens these itself, as `root`, before dropping privileges, and
    passes the open file descriptors to QEMU, which inherits them across the
    privilege drop and never opens the paths. The VMM user therefore needs no
    access to either. `/dev/vhost-vsock` in particular guards a host-global CID
    namespace where a compromised VMM could deny other sandboxes on the node
    their agent channel.

## Provisioning the host with kata-deploy

Set `rootless: true`. It requires `deploymentMode: job`, which is the default —
the DaemonSet has no privileged host-root stage to provision devices from, and
the chart refuses the combination rather than configuring sandboxes that could
not start.

!!! warning "Node requirement: udev"
    Provisioning is two independent steps — the device node is reconciled now,
    and a udev rule records that so it outlives the current boot. The second step
    assumes the node runs a udev implementation that reads `/etc/udev/rules.d`
    (`systemd-udevd` or eudev) and keeps `/etc` across reboots. That holds for
    every Kubernetes distribution the chart supports.

    Two kinds of node break the assumption, and neither reports an error, because
    writing the file succeeds either way:

    - busybox `mdev` reads `/etc/mdev.conf` and ignores udev rules entirely.
    - Images that rebuild `/etc` at boot, such as Bottlerocket, or that expect
      rules through machine configuration, such as Talos, discard the file.

    On such a node the devices are still reconciled, so sandboxes run — until the
    node reboots or the device is re-created, at which point the permissions
    revert and rootless sandboxes stop starting. Provision those hosts through
    whatever mechanism the image does support.

```bash title="$ helm install"
helm install kata-deploy \
    oci://quay.io/kata-containers/kata-deploy-charts/kata-deploy \
    --namespace kube-system \
    --set rootless=true
```

That does two things on every selected node:

1. Turns `rootless = true` on for each supported shim, through a
   `25-rootless.toml` drop-in.
2. Reconciles the devices in the table above and records the result as a udev
   rule, so it survives a reboot or the device being re-created:

    ```title="/etc/udev/rules.d/99-kata-containers-rootless-default.rules"
    KERNEL=="kvm", SUBSYSTEM=="misc", GROUP="kvm", MODE="0660"
    KERNEL=="sev", SUBSYSTEM=="misc", GROUP="kata-sev", MODE="0660"
    KERNEL=="uv", SUBSYSTEM=="misc", GROUP="kata-se", MODE="0660"
    ```

The rule only lists the devices the enabled shims actually need: `/dev/sev`
appears when an SNP shim is enabled, `/dev/uv` when `qemu-se-runtime-rs` is.
Groups are created with the node's own `groupadd --system`, so they land in the
node's system-GID range.

A device the node does not have is skipped — the chart enables every shim by
default, so a plain node with no `/dev/sev` is an ordinary case and not an
error. A device that *already* grants unprivileged access is left exactly as
the node has it, including a node that chose to use the other permission bits
or a group of its own.

!!! note "What uninstall reverts"
    Uninstall removes the udev rule. It leaves the groups and the live device
    modes alone, for the same reason it leaves host kernel modules loaded: both
    are host-global, another installation may be relying on them, and an empty
    group grants nobody anything.

## Provisioning the host by hand

Outside Kubernetes, or on a node kata-deploy does not manage, do the same two
steps yourself: create the group, then give the device group read/write and make
it stick.

=== "AMD SEV-SNP"
    ```bash
    getent group kata-sev >/dev/null || sudo groupadd --system kata-sev
    sudo chgrp kata-sev /dev/sev
    sudo chmod 0660 /dev/sev
    ```

    ```bash title="$ ls -l /dev/sev"
    crw-rw---- 1 root kata-sev 10, 124 /dev/sev
    ```

=== "IBM Secure Execution"
    ```bash
    getent group kata-se >/dev/null || sudo groupadd --system kata-se
    sudo chgrp kata-se /dev/uv
    sudo chmod 0660 /dev/uv
    ```

    ```bash title="$ ls -l /dev/uv"
    crw-rw---- 1 root kata-se 10, 262 /dev/uv
    ```

=== "KVM"
    ```bash
    getent group kvm >/dev/null || sudo groupadd --system kvm
    sudo chgrp kvm /dev/kvm
    sudo chmod 0660 /dev/kvm
    ```

    ```bash title="$ ls -l /dev/kvm"
    crw-rw---- 1 root kvm 10, 232 /dev/kvm
    ```

!!! warning "`chmod` alone does not survive a reboot"
    `/dev` is a `devtmpfs`, so the commands above last only until the device is
    re-created. Add a udev rule for each device you changed, named so it sorts
    after the distribution's own rules:

    ```title="/etc/udev/rules.d/99-kata-rootless.rules"
    KERNEL=="sev", SUBSYSTEM=="misc", GROUP="kata-sev", MODE="0660"
    ```

## Intel TDX and the quote generation service

TDX attestation reaches QGS through one of two transports, and only one of them
has host permissions to think about.

AF_VSOCK
:   No host filesystem pathname, so nothing to own and nothing to provision.
    This is the supported path, and the one Kata's CI exercises.

Unix socket
:   Needs the VMM user to traverse every parent directory *and* to have **write**
    permission on the socket itself — connecting to a Unix socket is a write.
    Mode `0755` is therefore not sufficient, which is a common surprise.

When a UDS is configured, the shim validates it before QEMU starts and adds the
socket's owning GID to the VMM user's supplementary groups when group write
permission is what grants access. It does not change the socket's owner or mode:
QGS ownership is the deployment's to set, and kata-deploy does not manage it.

A host running QGS as `qgsd:qgsd` therefore has to grant group write permission
on the socket for a VMM user to connect through membership in `qgsd`.

!!! tip
    Prefer AF_VSOCK unless something in your deployment rules it out. It has no
    filesystem permissions to get wrong, and it needs no host provisioning at
    all.

## Enabling rootless without kata-deploy

Either set the flag in the hypervisor section of `configuration.toml`:

```toml title="configuration-qemu-runtime-rs.toml"
[hypervisor.qemu]
rootless = true
```

…or set the Kubernetes annotation on the pod:

```yaml
io.katacontainers.config.hypervisor.rootless: "true"
```

!!! note
    Shipped configurations do not allow the `rootless` annotation, so it has to
    be added to the hypervisor's `enable_annotations` list before a pod can use
    it. Setting the flag in the configuration file needs no such change.

## Limitations

1. Only the VMM process runs unprivileged. The Kata shim and `virtiofsd` still
   run as `root`.
2. Supported for the runtime-rs QEMU shims only; see
   [Supported hypervisors](#supported-hypervisors).
3. Passing host devices into the guest (`virtio-blk`, `virtio-scsi`) fails
   unless the unprivileged user can open them. The same contract applies: a more
   permissive mode fixes it, at a security cost worth weighing first.
4. VFIO passthrough works only on a host that offers the IOMMUFD per-device
   character device. There the shim opens `/dev/iommu` and
   `/dev/vfio/devices/vfioN` as `root` and passes the descriptors to QEMU, so no
   host provisioning is involved and nothing needs to change for rootless.

    Two cases fall outside that and are unsupported:

    - **Legacy VFIO groups**, on a host with no per-device cdev. QEMU is given
      `host=<BDF>` and opens `/dev/vfio/<group>` itself, which is `root:root`
      `0600` by default.
    - **`vfio-ap` on s390x**, where QEMU is given `sysfsdev=<path>` and opens the
      mediated device itself.

    kata-deploy deliberately does not provision these. An IOMMU group number is
    assigned at runtime, so no rule can name one ahead of time — covering them
    would mean granting a group access to every VFIO device on the node through a
    blanket `SUBSYSTEM=="vfio"` match. That is a far wider grant than the
    single-purpose one this feature makes for `/dev/sev` or `/dev/uv`, and it is
    not made on a node's behalf.
