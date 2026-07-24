#!/usr/bin/env bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

VMM_CONFIGS="qemu fc"

# shellcheck disable=SC2269
GO_VERSION=${GO_VERSION}
# shellcheck disable=SC2269
RUST_VERSION=${RUST_VERSION}

RUNTIME_CHOICE="${RUNTIME_CHOICE:-both}"
DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="${SHIM_V2_CONTAINER_BUILDER:-$(get_shim_v2_image_name)}"

EXTRA_OPTS="${EXTRA_OPTS:-""}"

case "${RUNTIME_CHOICE}" in
	"go"|"rust"|"both")
		echo "Building ${RUNTIME_CHOICE} runtime(s)"
		;;
	*)
		echo "Invalid option for RUNTIME_CHOICE: ${RUNTIME_CHOICE}"
		exit 1
		;;
esac

# Variants (targets) that build a measured rootfs as of now are:
# - rootfs-image (the base image, measured; root hash labelled "base")
# - rootfs-image-coco-extension (the CoCo guest-components extension)
# - rootfs-image-nvidia (the driver-agnostic NVIDIA boot image)
# - rootfs-image-nvidia-gpu-extension (the driver-versioned gpu extension)
#
# Both runtimes now boot the composable split layout (base image plus
# cold-plugged extensions), so the Go and Rust `make` invocations map the same
# make vars to the same root hashes:
# - the measured base image (@KERNELVERITYPARAMS@) plus the CoCo extension
#   (@COCOVERITYPARAMS@), and for NVIDIA the nvidia base image
#   (@KERNELVERITYPARAMS_NV@) plus the gpu extension
#   (@NVIDIAGPUEXTENSIONVERITYPARAMS@).
#
# shellcheck disable=SC2154
root_hash_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"

# read_verity_param <variant-label> <make-var-name>
# Emits " VAR=value" if the matching root_hash_<variant>.txt exists, else nothing.
read_verity_param() {
	local variant="$1"
	local param_var="$2"
	local root_hash_file="${root_hash_dir}/root_hash_${variant}.txt"
	[[ -f "${root_hash_file}" ]] || return 0

	# root_hash_*.txt contains a single kernel_verity_params line.
	local root_measure_config
	IFS= read -r root_measure_config < "${root_hash_file}"
	root_measure_config="${root_measure_config%$'\r'}"
	[[ -n "${root_measure_config}" ]] || die "Empty kernel verity params in ${root_hash_file}"

	printf ' %s=%s' "${param_var}" "${root_measure_config}"
}

# Both runtimes boot the composable split layout, so they share the same
# root-hash-to-make-var mapping: the measured base image plus the CoCo extension,
# and for NVIDIA the nvidia base image plus the gpu extension.  qemu-nvidia-cpu
# (Go) also boots the driver-agnostic nvidia base image verity-backed and reads
# it through its own KERNELVERITYPARAMS_NV_BASE var.
SPLIT_EXTRA_OPTS="$(read_verity_param "base" "KERNELVERITYPARAMS")"
SPLIT_EXTRA_OPTS+="$(read_verity_param "coco-extension" "COCOVERITYPARAMS")"
SPLIT_EXTRA_OPTS+="$(read_verity_param "nvidia" "KERNELVERITYPARAMS_NV")"
SPLIT_EXTRA_OPTS+="$(read_verity_param "nvidia-gpu-extension" "NVIDIAGPUEXTENSIONVERITYPARAMS")"

# runtime-rs (Rust): composable split layout.
RUST_EXTRA_OPTS="${SPLIT_EXTRA_OPTS}"

# runtime (Go): also the composable split layout, plus the nvidia base hash in
# KERNELVERITYPARAMS_NV_BASE consumed by the qemu-nvidia-cpu config.
GO_EXTRA_OPTS="${SPLIT_EXTRA_OPTS}"
GO_EXTRA_OPTS+="$(read_verity_param "nvidia" "KERNELVERITYPARAMS_NV_BASE")"

# shellcheck disable=SC2154,SC2086
docker pull "${container_image}" || \
	(docker build \
		--build-arg GO_VERSION="${GO_VERSION}" \
		--build-arg RUST_VERSION="${RUST_VERSION}" \
		-t "${container_image}" \
		"${script_dir}" && \
	 push_to_registry "${container_image}")

arch=${ARCH:-$(uname -m)}
if [[ "${arch}" = "ppc64le" ]]; then
	arch="ppc64"
fi

case "${RUNTIME_CHOICE}" in
	"rust"|"both")
		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			--env ARCH="${ARCH}" \
			-w "${repo_root_dir}/src/runtime-rs" \
			--user "$(id -u)":"$(id -g)" \
			"${container_image}" \
			bash -c "make clean-generated-files && make PREFIX=${PREFIX} QEMUCMD=qemu-system-${arch} ${EXTRA_OPTS}${RUST_EXTRA_OPTS}"

		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
		        --env ARCH="${ARCH}" \
			-w "${repo_root_dir}/src/runtime-rs" \
			--user "$(id -u)":"$(id -g)" \
			"${container_image}" \
			bash -c "make PREFIX='${PREFIX}' DESTDIR='${DESTDIR}' ${EXTRA_OPTS}${RUST_EXTRA_OPTS} install"
		;;
esac

# When STATIC_RUNTIME=yes the Go host binaries (kata-runtime,
# containerd-shim-kata-v2 and kata-monitor) are built as fully static,
# cgo-free executables so the payload runs on musl-only hosts that have no
# glibc dynamic loader. Default builds are unchanged.
if [[ "${STATIC_RUNTIME:-}" == "yes" ]]; then
	GO_EXTRA_OPTS+=" STATIC=yes"
fi

case "${RUNTIME_CHOICE}" in
	"go"|"both")
		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			-w "${repo_root_dir}/src/runtime" \
			--user "$(id -u)":"$(id -g)" \
			--env GOMODCACHE=/opt/.cache/gomod \
			"${container_image}" \
			bash -c "make clean-generated-files && make PREFIX=${PREFIX} QEMUCMD=qemu-system-${arch} ${EXTRA_OPTS}${GO_EXTRA_OPTS}"

		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			-w "${repo_root_dir}/src/runtime" \
			--user "$(id -u)":"$(id -g)" \
			--env GOMODCACHE=/opt/.cache/gomod \
			"${container_image}" \
			bash -c "make PREFIX='${PREFIX}' DESTDIR='${DESTDIR}' ${EXTRA_OPTS}${GO_EXTRA_OPTS} install"
		;;
esac

for vmm in ${VMM_CONFIGS}; do
	for config_file in "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-${vmm}"*.toml; do
		if [[ -f "${config_file}" ]]; then
			if [[ "${ARCH}" == "ppc64le" ]]; then
				# On ppc64le, replace image line with initrd line
				sed -i -e 's|^image = .*|initrd = "'"${PREFIX}"'/share/kata-containers/kata-containers-initrd.img"|' "${config_file}"
			fi
		fi
	done
done

pushd "${DESTDIR}/${PREFIX}/share/defaults/kata-containers"
	ln -sf "configuration-qemu.toml" configuration.toml
popd
