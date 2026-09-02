# Packaging scripts

This directory contains useful packaging scripts.

## `configure-hypervisor.sh`

This script generates the official set of QEMU-based hypervisor build
configuration options. All repositories that need to build a hypervisor
from source **MUST** use this script to ensure the hypervisor is built
in a known way since using a different set of options can impact many
areas including performance, memory footprint and security.

Example usage:

```
  $ configure-hypervisor.sh qemu
```

## `gen-initdata-image.sh`

This script packs an initdata document into an initdata disk image, which is one
of the two forms a node can hand to a confidential guest through the
`initdata_path` configuration option. See
[Providing initdata from the node](../../../docs/how-to/how-to-provide-node-level-initdata.md)
for when to prefer an image over a plain document.

Example usage:

```
  $ gen-initdata-image.sh -o initdata.img initdata.toml
  $ gen-initdata-image.sh -d initdata.img
```
