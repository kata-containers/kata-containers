# IGVM measured-boot image build

This target builds a single **IGVM** (Independent Guest Virtual Machine) image
that bundles the confidential guest kernel, the SEV-SNP OVMF firmware and the
measured kernel command line, plus its launch measurement. Booting from this
image (see [the IGVM how-to](../../../../docs/how-to/how-to-run-kata-containers-with-IGVM.md))
gives a deterministic, offline-computable launch measurement instead of one that
must be reconstructed from discrete kernel/firmware/cmdline inputs.

## Inputs

| env var | default | meaning |
| --- | --- | --- |
| `igvm_kernel` | `…/share/kata-containers/vmlinuz-confidential.container` | guest kernel, from the kernel build step |
| `igvm_firmware` | `…/share/ovmf/AMDSEV.fd` | SEV-SNP firmware, from the `ovmf-sev` build step |
| `igvm_cmdline` | _(required)_ | **measured** kernel command line; must contain the rootfs `root=` and dm-verity roothash |

## Output

- `…/share/kata-containers/kata.igvm` — the measured image
- `…/share/kata-containers/kata.igvm.measurement` — the expected launch measurement

## Cross-repo dependency

The build runs the **`steep` igvm-tools** CLI (the `igvm-tools` crate in the
[`steep`](https://github.com/lunal-dev/steep) repo) for both the IGVM build and
the measurement:

```
igvm-tools build --platform snp --firmware AMDSEV.fd \
    --kernel vmlinuz --cmdline "<measured cmdline>" \
    --output kata.igvm --manifest kata.igvm.manifest.json
```

`--cmdline` wraps the plain guest `vmlinuz` into a UKI (via `ukify`) so the
command line — including the dm-verity root hash — is embedded and measured. The
command prints the SNP launch digest, which is captured into
`kata.igvm.measurement`. (The `--cmdline`/`--initrd` options were added to steep
igvm-tools to support this; steep's own flow bakes the cmdline via mkosi
instead.)

The builder image pins steep via `.externals.igvm.{url,version}` in
`versions.yaml`. The kata-side wiring is opt-in: with no `igvm` path set in the
runtime config, guests still boot from the discrete kernel/firmware assets.
