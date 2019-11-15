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
