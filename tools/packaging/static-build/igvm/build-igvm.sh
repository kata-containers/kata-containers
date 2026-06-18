#!/usr/bin/env bash
#
# Copyright (c) 2026 Lunal / Confidential AI
#
# SPDX-License-Identifier: Apache-2.0
#
# Runs inside the igvm builder container. Bundles the guest kernel + SEV-SNP
# firmware + measured command line into a single IGVM image (steep's igvm-tools
# wraps the kernel into a UKI so the command line is measured) and records its
# launch measurement so attestation can be validated offline.

set -o errexit
set -o nounset
set -o pipefail

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}

igvm_kernel=${igvm_kernel:?igvm_kernel (path to guest vmlinuz) must be set}
igvm_firmware=${igvm_firmware:?igvm_firmware (path to AMDSEV.fd) must be set}
# The command line is MEASURED: it must include the rootfs root= and the
# dm-verity roothash so the firmware+kernel+cmdline+rootfs attest as one digest.
igvm_cmdline=${igvm_cmdline:?igvm_cmdline must be set}

[[ -f "${igvm_kernel}" ]] || { echo "guest kernel not found: ${igvm_kernel}" >&2; exit 1; }
[[ -f "${igvm_firmware}" ]] || { echo "firmware not found: ${igvm_firmware}" >&2; exit 1; }

install_dir="${DESTDIR}${PREFIX}/share/kata-containers"
igvm_out="${install_dir}/kata.igvm"
manifest_out="${install_dir}/kata.igvm.manifest.json"
measurement_out="${install_dir}/kata.igvm.measurement"

mkdir -p "${install_dir}"

# steep's igvm-tools builds the SEV-SNP IGVM (firmware + kernel + measured
# cmdline as a UKI + SNP VMSA + measured directives) and prints the launch
# digest. `--cmdline` wraps the plain vmlinuz into a UKI via ukify so the
# command line (with the dm-verity roothash) is part of the measurement.
igvm-tools build \
	--platform snp \
	--firmware "${igvm_firmware}" \
	--kernel "${igvm_kernel}" \
	--cmdline "${igvm_cmdline}" \
	--output "${igvm_out}" \
	--manifest "${manifest_out}" \
	| tee "${measurement_out}"

echo "Built measured IGVM image: ${igvm_out}"
echo "Manifest: ${manifest_out}"
echo "Launch measurement: $(cat "${measurement_out}")"
