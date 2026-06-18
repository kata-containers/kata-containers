#!/usr/bin/env bash
#
# Copyright (c) 2026 Lunal / Confidential AI
#
# SPDX-License-Identifier: Apache-2.0
#
# Build a measured IGVM image that bundles the confidential guest kernel, the
# SEV-SNP OVMF firmware and the (measured) kernel command line into a single
# file, together with its launch measurement. The heavy lifting is done by the
# `steep` igvm-tools, run inside a builder container.

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly igvm_builder="${script_dir}/build-igvm.sh"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}

container_image="${IGVM_CONTAINER_BUILDER:-$(get_igvm_image_name)}"

# Inputs: the confidential guest kernel and the SEV-SNP firmware, as installed
# by the kernel and ovmf-sev build steps into the same DESTDIR/PREFIX tree.
igvm_kernel="${igvm_kernel:-${DESTDIR}${PREFIX}/share/kata-containers/vmlinuz-confidential.container}"
igvm_firmware="${igvm_firmware:-${DESTDIR}${PREFIX}/share/ovmf/AMDSEV.fd}"
# The command line is part of the MEASURED image and must include the rootfs
# root= and dm-verity roothash so the whole stack attests as a single digest.
# It is supplied by the caller (the rootfs build knows the roothash).
igvm_cmdline="${igvm_cmdline:-}"

steep_repo="${steep_repo:-$(get_from_kata_deps ".externals.igvm.url")}"
steep_version="${steep_version:-$(get_from_kata_deps ".externals.igvm.version")}"

[[ -n "${steep_repo}" ]] || die "failed to get steep (igvm-tools) repo"
[[ -n "${steep_version}" ]] || die "failed to get steep (igvm-tools) version"

docker pull "${container_image}" || \
	(docker build \
		--build-arg STEEP_REPO="${steep_repo}" \
		--build-arg STEEP_VERSION="${steep_version}" \
		-t "${container_image}" "${script_dir}" && \
	# No-op unless PUSH_TO_REGISTRY is exported as "yes"
	push_to_registry "${container_image}")

# shellcheck disable=SC2154
docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${PWD}" \
	--env DESTDIR="${DESTDIR}" --env PREFIX="${PREFIX}" \
	--env igvm_kernel="${igvm_kernel}" \
	--env igvm_firmware="${igvm_firmware}" \
	--env igvm_cmdline="${igvm_cmdline}" \
	--user "$(id -u)":"$(id -g)" \
	"${container_image}" \
	bash -c "${igvm_builder}"
