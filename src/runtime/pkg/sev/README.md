# AMD SEV confidential guest utilities

This package provides utilities for launching AMD SEV/SEV-ES confidential
guests.

## Calculating expected launch digests

The `CalculateLaunchDigest` and `CalculateSEVESLaunchDigest` function can be
used to calculate the expected SHA-256 of an SEV/SEV-ES confidential guest
given its firmware, kernel, initrd, and kernel command-line.

### Unit test data

The [`testdata`](testdata) directory contains file used for testing
launch digest calculation.
