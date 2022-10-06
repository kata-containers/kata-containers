# AMD SEV confidential guest utilities

This package provides utilities for launching AMD SEV confidential guests.

## Calculating expected launch digests

The `CalculateLaunchDigest` function can be used to calculate the expected
SHA-256 of an SEV confidential guest given its firmware, kernel, initrd, and
kernel command-line.

### Unit test data

The [`testdata`](testdata) directory contains file used for testing
`CalculateLaunchDigest`.
