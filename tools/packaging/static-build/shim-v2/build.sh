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
CC=""

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

# shellcheck disable=SC2154
[[ "${CROSS_BUILD}" == "true" ]] && container_image_bk="${container_image}" && container_image="${container_image}-cross-build"

# Variants (targets) that build a measured rootfs as of now are:
# - rootfs-image (the base image, measured; root hash labelled "base")
# - rootfs-image-confidential (monolithic CoCo image, root hash "confidential")
# - rootfs-image-coco-extension
# - rootfs-image-nvidia-gpu / rootfs-image-nvidia-gpu-confidential (monolithic)
# - rootfs-image-nvidia (the driver-agnostic NVIDIA boot image)
# - rootfs-image-nvidia-gpu-extension (the driver-versioned gpu extension)
#
# Both the CoCo and the NVIDIA GPU configs come in two flavours during the
# transition to split images, and the two runtimes are built in separate `make`
# invocations so each maps the same make var to a different root hash:
# - runtime-rs (Rust) uses the split layout: the measured base image
#   (@KERNELVERITYPARAMS@) plus the CoCo extension (@COCOVERITYPARAMS@), and for
#   NVIDIA the nvidia base image (@KERNELVERITYPARAMS_NV@) plus the gpu extension
#   (@NVIDIAGPUEXTENSIONVERITYPARAMS@).
# - runtime (Go) still uses the monolithic images: the confidential image
#   (@KERNELVERITYPARAMS@) and, for NVIDIA, nvidia-gpu (@KERNELVERITYPARAMS_NV@)
#   and nvidia-gpu-confidential (@KERNELVERITYPARAMS_CONFIDENTIAL_NV@).
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

# The NVIDIA GPU verity params differ per runtime: runtime-rs boots the
# composable split layout (nvidia base + gpu extension) while the Go runtime
# still boots the monolithic nvidia-gpu / nvidia-gpu-confidential images.  Since
# both runtimes reuse KERNELVERITYPARAMS_NV but map it to different root hashes,
# the reads live in each runtime's own EXTRA_OPTS rather than the shared one.

# runtime-rs (Rust): split images -> base hash + CoCo extension hash, plus the
# nvidia base and gpu extension hashes for the NVIDIA GPU configs.
RUST_EXTRA_OPTS="$(read_verity_param "base" "KERNELVERITYPARAMS")"
RUST_EXTRA_OPTS+="$(read_verity_param "coco-extension" "COCOVERITYPARAMS")"
RUST_EXTRA_OPTS+="$(read_verity_param "nvidia" "KERNELVERITYPARAMS_NV")"
RUST_EXTRA_OPTS+="$(read_verity_param "nvidia-gpu-extension" "NVIDIAGPUEXTENSIONVERITYPARAMS")"

# runtime (Go): monolithic confidential image -> confidential hash, plus the
# monolithic nvidia-gpu / nvidia-gpu-confidential hashes for the NVIDIA configs.
GO_EXTRA_OPTS="$(read_verity_param "confidential" "KERNELVERITYPARAMS")"
GO_EXTRA_OPTS+="$(read_verity_param "nvidia-gpu" "KERNELVERITYPARAMS_NV")"
GO_EXTRA_OPTS+="$(read_verity_param "nvidia-gpu-confidential" "KERNELVERITYPARAMS_CONFIDENTIAL_NV")"
# qemu-nvidia-cpu (Go) boots the driver-agnostic nvidia base image verity-backed;
# it needs the "nvidia" base hash in its own var since KERNELVERITYPARAMS_NV
# above is the monolithic nvidia-gpu hash.
GO_EXTRA_OPTS+="$(read_verity_param "nvidia" "KERNELVERITYPARAMS_NV_BASE")"

# shellcheck disable=SC2154,SC2086
docker pull "${container_image}" || \
	(docker ${BUILDX} build ${PLATFORM}  \
		--build-arg GO_VERSION="${GO_VERSION}" \
		--build-arg RUST_VERSION="${RUST_VERSION}" \
		-t "${container_image}" \
		"${script_dir}" && \
	 push_to_registry "${container_image}")

arch=${ARCH:-$(uname -m)}
GCC_ARCH=${arch}
if [[ "${arch}" = "ppc64le" ]]; then
	GCC_ARCH="powerpc64le"
	arch="ppc64"
fi

case "${RUNTIME_CHOICE}" in
	"rust"|"both")
		#Build rust project using cross build musl image to speed up
		[[ "${CROSS_BUILD}" == "true" && ${ARCH} != "s390x" ]] && container_image="messense/rust-musl-cross:${GCC_ARCH}-musl" && CC=${GCC_ARCH}-unknown-linux-musl-gcc

		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			--env CROSS_BUILD="${CROSS_BUILD}" \
			--env ARCH="${ARCH}" \
			--env CC="${CC}" \
			-w "${repo_root_dir}/src/runtime-rs" \
			--user "$(id -u)":"$(id -g)" \
			"${container_image}" \
			bash -c "make clean-generated-files && make PREFIX=${PREFIX} QEMUCMD=qemu-system-${arch} ${EXTRA_OPTS}${RUST_EXTRA_OPTS}"

		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			--env CROSS_BUILD="${CROSS_BUILD}" \
		        --env ARCH="${ARCH}" \
		        --env CC="${CC}" \
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
		[[ "${CROSS_BUILD}" == "true" ]] && container_image="${container_image_bk}-cross-build"

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
