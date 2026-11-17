# Firecracker read-only block attachments

Firecracker negotiates a virtio disk's read-only feature at boot. Replacing a
disk's backing file cannot change that feature. Kata therefore reserves separate
writable and read-only placeholder slots before starting a sandbox.

Both the Go and Rust runtimes provide eight attachment slots, separate from the
Kata guest OS root disk. Set `block_device_read_only_slots` in the Firecracker
hypervisor configuration before creating the sandbox. The default, `1`, reserves
the last slot for read-only attachments and leaves seven writable slots. The
accepted range is 0 through 7, ensuring at least one writable slot remains for a
container root disk. Setting `0` disables read-only attachments.

There is no fallback between the two pools: exhausting either pool fails the
attachment. Container root disks and read-only image devices consume attachment
slots too. Budget enough slots for all concurrently attached devices in a sandbox.
The default supports one independently attached read-only device; workloads needing
more must increase the setting before creating the sandbox.

The placeholder drives also consume MMIO interrupt resources on x86_64. With
Firecracker v1.17.0, one guest OS root disk and one vsock device, eight attachment
slots leave room for seven NICs; an eighth NIC prevents the VM from starting.
The previous five-slot Rust pool allowed ten NICs with the same configuration.
Go already used eight attachment slots. Capacity depends on the Firecracker
version and other configured devices.

In runtime-rs, the EROFS rootfs path requests read-only block devices. This branch
combines multiple EROFS layers into a VMDK-backed device, but the Firecracker
attachment path does not interpret that VMDK layout. Increasing the read-only
slot count does not add support for this multi-layer format. Supported raw EROFS
image devices still consume read-only slots alongside other read-only volumes.

Changing the runtime configuration does not change a running sandbox's disk
layout. Upgrading the runtime does not make previously writable VMM disks
read-only: recreate affected sandboxes to establish the new protection.
Go sandboxes saved by older runtimes restore with zero read-only slots and reject
new read-only attachments until the sandbox is recreated.

## Scope

This protects block attachments whose read-only intent is provided to the Kata
device layer. It does not turn OCI `root.readonly` into a read-only root block
device. The guest runtime prepares mount points and device nodes before applying
that container filesystem restriction.

All containers in a sandbox share one guest. Mixed read-only and writable requests
for the same shared block device are unsupported. When a request reuses an existing
attachment, it retains the mode chosen by the first attachment. If the first
attachment is read-only, a later writable request still receives a read-only
guest disk and cannot write to it; a writable mount may fail. Previously,
Firecracker's writable attachment slots did not enforce this restriction. If the
first attachment is writable, mounting it read-only in another container does not
provide device-level read-only protection. No mode-conflict rejection policy is
implemented.
