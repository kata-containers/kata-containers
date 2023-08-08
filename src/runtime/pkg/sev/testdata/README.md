# sev/testdata

The `ovmf_suffix.bin` contains the last 4KB of the `OVMF.fd` binary from edk2's
`OvmfPkg/AmdSev/AmdSevX64.dsc` build.  To save space, we committed only the
last 4KB instead of the the full 4MB binary.

The end of the file contains a GUIDed footer table with entries that hold the
SEV-ES AP reset vector address, which is needed in order to compute VMSAs for
SEV-ES guests.
