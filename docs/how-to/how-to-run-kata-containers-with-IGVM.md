# Booting Kata Containers from a measured IGVM image (QEMU)

## Overview

By default, a confidential Kata guest is assembled at launch from three separate
host assets — a guest kernel (`vmlinuz`), a UEFI firmware (`OVMF`/`AMDSEV.fd`)
and a kernel command line — and, for AMD SEV-SNP, the firmware measures the
kernel/initrd/command line into the launch measurement via QEMU's
`kernel-hashes=on`. To know whether a given guest will pass attestation, the
tenant has to reconstruct that measurement by hand from those inputs.

**IGVM** (Independent Guest Virtual Machine) collapses those inputs into a
single file that bundles the firmware, the kernel and the (measured) command
line together with the directives that define the guest's initial memory and CPU
state. QEMU (9.1+) loads it via an `igvm-cfg` object, and the launch measurement
is fully determined by the IGVM image — so it can be computed **offline** with
an IGVM measurement tool (steep's `igvm-tools measure`, or the upstream
[`igvmmeasure`](https://github.com/microsoft/igvm)) instead of being
reconstructed from discrete assets.

This document covers booting a QEMU Kata guest from an IGVM image. It assumes
you already have a working SEV-SNP setup; see
[How to run Kata Containers with AMD SEV-SNP VMs](how-to-run-kata-containers-with-SNP-VMs.md)
for host prerequisites, kernel/OVMF builds and the base SNP configuration.

## What changes when booting from IGVM

| | Discrete boot (default) | IGVM boot |
| --- | --- | --- |
| Host assets | `kernel` + `firmware` (+ `firmware_volume`) | one `igvm` image |
| Command line | passed at launch via `-append` | baked into the image and measured |
| Kernel/firmware measurement | OVMF measures them at launch (`kernel-hashes=on`) | fixed by the IGVM directives |
| Expected launch measurement | reconstructed by the tenant | `igvm-tools measure kata.igvm` (offline, deterministic) |
| Rootfs | separate `image` disk | separate `image` disk (unchanged) |

The SEV-SNP launch parameters that are **not** part of the measured image —
`host-data` (the initdata digest), the guest `policy`, and the optional
`id-block`/`id-auth` — are still applied exactly as before. Attestation report
retrieval and verification remain the tenant's responsibility and are unchanged.

> **Fixed command line.** Because the kernel command line is part of the measured
> image, it is fixed at build time. Per-sandbox kernel parameters are not applied
> in IGVM mode (they would change the measurement). Use
> [initdata](https://github.com/confidential-containers) for per-tenant
> configuration, which is delivered over the separate, measured `host-data`
> channel.

## Building the IGVM image

The IGVM image bundles the confidential guest `vmlinuz`, the SEV-SNP OVMF
firmware and the kernel command line (including the dm-verity root hash of the
rootfs image, so the whole stack attests as a single digest). The command line
is wrapped into a measured UKI by steep's `igvm-tools`. See
`tools/packaging/static-build/igvm/` for the build target.

After building, print the launch measurement that attestation should expect:

```sh
igvm-tools measure /opt/kata/share/kata-containers/kata.igvm
```

## Enabling IGVM boot

IGVM boot is **opt-in**: with no `igvm` path set, the guest boots from the
discrete kernel/firmware assets as before. There are two ways to enable it.

### 1. Configuration file

Point the `igvm` key in the SNP configuration at the image:

```toml
[hypervisor.qemu]
# kernel/firmware are ignored for boot when igvm is set
igvm = "/opt/kata/share/kata-containers/kata.igvm"
image = "/opt/kata/share/kata-containers/kata-containers-confidential.img"
confidential_guest = true
sev_snp_guest = true
```

When `igvm` is set, Kata passes the image to QEMU as
`-object igvm-cfg,id=igvm0,file=...` linked from the machine
(`-machine ...,igvm-cfg=igvm0`), and emits no `-kernel`, `-initrd`, `-append` or
`-bios`. The rootfs `image` is still attached as a separate disk.

### 2. Per-sandbox annotation

If `io.katacontainers.config.hypervisor.igvm` is in the runtime's enabled
annotations, an individual pod can select the image:

```yaml
metadata:
  annotations:
    io.katacontainers.config.hypervisor.igvm: /opt/kata/share/kata-containers/kata.igvm
```

An optional `io.katacontainers.config.hypervisor.igvm_hash` (SHA-512) is verified
against the file before boot.

## Verifying

A guest booted from IGVM should show the `igvm-cfg` object and no discrete
kernel/bios on the QEMU command line:

```sh
ps -ef | grep qemu-system-x86_64 | grep -o -- '-object igvm-cfg[^ ]*'
# -object igvm-cfg,id=igvm0,file=/opt/kata/share/kata-containers/kata.igvm
```

The launch measurement reported in the SEV-SNP attestation report must equal the
value printed by `igvm-tools measure` for the same image.
