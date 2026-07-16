# Runtime-rs configuration drop-in examples

Runtime-rs loads optional TOML fragments from a `config.d/` directory next to
the base configuration file. Files are merged in lexicographic order; later
files override scalars and tables, while `guest_extension_images` arrays are
**appended** so drop-ins can add extensions without repeating base entries.

## NVIDIA devkit extension

Use `25-devkit.toml.example` with any NVIDIA runtime-rs profile:

- `configuration-qemu-nvidia-cpu-runtime-rs.toml`
- `configuration-qemu-nvidia-gpu-runtime-rs.toml`
- `configuration-qemu-nvidia-gpu-snp-runtime-rs.toml`
- `configuration-qemu-nvidia-gpu-tdx-runtime-rs.toml`

Copy the example into the matching `config.d/` directory and adjust `path` if
you use a non-default install prefix. Set `verity_params` to the single-line
contents of `root_hash_nvidia-devkit-extension.txt` from the same build as the image.
After a normal runtime-rs install, the example lives at
`/opt/kata/share/defaults/kata-containers/runtime-rs/drop-in-examples/25-devkit.toml.example`.

When `debug: true` is set in kata-deploy for an NVIDIA runtime-rs shim, kata-deploy
writes `20-debug.toml` with `debug_console_enabled = true` and the devkit extension
entry (including `verity_params` from `root_hash_nvidia-devkit-extension.txt` when
`root_hash_nvidia-devkit-extension.txt` is installed).

See [NVIDIA devkit extension](../../../../docs/how-to/how-to-build-and-deploy-local-artifacts.md#nvidia-devkit-extension)
for building and installing `kata-containers-nvidia-devkit-extension.img`.
