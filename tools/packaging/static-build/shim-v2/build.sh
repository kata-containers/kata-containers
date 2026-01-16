#!/usr/bin/env bash
#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

VMM_CONFIGS="qemu fc"

GO_VERSION=${GO_VERSION}
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

[ "${CROSS_BUILD}" == "true" ] && container_image_bk="${container_image}" && container_image="${container_image}-cross-build"
if [ "${MEASURED_ROOTFS}" == "yes" ]; then
	info "Enable rootfs measurement config"

	# Two VARIANTS (targets) that build a measured rootfs as of now are:
	# - rootfs-image-confidential
	# - rootfs-image-nvidia-gpu-confidential
	#
	found_any=""
	for variant in confidential nvidia-gpu-confidential; do
		root_hash_file="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/root_hash_${variant}.txt"
		[ -f "$root_hash_file" ] || \
			die "Root hash file for measured rootfs ${variant} not found at ${root_hash_file}"

		found_any="yes"

		root_hash=$(sed -e 's/Root hash:\s*//g;t;d' "${root_hash_file}")
		root_measure_config="rootfs_verity.scheme=dm-verity rootfs_verity.hash=${root_hash}"

		[ "${variant}" == "confidential" ] && EXTRA_OPTS+=" ROOTMEASURECONFIG=\"${root_measure_config}\""
		[ "${variant}" == "nvidia-gpu-confidential" ] && EXTRA_OPTS+=" ROOTMEASURECONFIG_NV=\"${root_measure_config}\""

	done
	[ -z "${found_any}" ] && die "No root hash files found for shim-v2 with MEASURED_ROOTFS support, needs a rootfs with MEASURED_ROOTFS support"
fi

docker pull ${container_image} || \
	(docker ${BUILDX} build ${PLATFORM}  \
		--build-arg GO_VERSION="${GO_VERSION}" \
		--build-arg RUST_VERSION="${RUST_VERSION}" \
		-t "${container_image}" \
		"${script_dir}" && \
	 push_to_registry "${container_image}")

arch=${ARCH:-$(uname -m)}
GCC_ARCH=${arch}
if [ ${arch} = "ppc64le" ]; then
	GCC_ARCH="powerpc64le"
	arch="ppc64"
fi

case "${RUNTIME_CHOICE}" in
	"rust"|"both")
		#Build rust project using cross build musl image to speed up
		[[ "${CROSS_BUILD}" == "true" && ${ARCH} != "s390x" ]] && container_image="messense/rust-musl-cross:${GCC_ARCH}-musl" && CC=${GCC_ARCH}-unknown-linux-musl-gcc

		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			--env CROSS_BUILD=${CROSS_BUILD} \
			--env ARCH=${ARCH} \
			--env CC="${CC}" \
			-w "${repo_root_dir}/src/runtime-rs" \
			--user "$(id -u)":"$(id -g)" \
			"${container_image}" \
			bash -c "make clean-generated-files && make PREFIX=${PREFIX} QEMUCMD=qemu-system-${arch}"

		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			--env CROSS_BUILD=${CROSS_BUILD} \
		        --env ARCH=${ARCH} \
		        --env CC="${CC}" \
			-w "${repo_root_dir}/src/runtime-rs" \
			--user "$(id -u)":"$(id -g)" \
			"${container_image}" \
			bash -c "make PREFIX="${PREFIX}" DESTDIR="${DESTDIR}" install"
		;;
esac

case "${RUNTIME_CHOICE}" in
	"go"|"both")
		[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image_bk}-cross-build"

		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			-w "${repo_root_dir}/src/runtime" \
			--user "$(id -u)":"$(id -g)" \
			"${container_image}" \
			bash -c "make clean-generated-files && make PREFIX=${PREFIX} QEMUCMD=qemu-system-${arch} ${EXTRA_OPTS}"

		docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
			-w "${repo_root_dir}/src/runtime" \
			--user "$(id -u)":"$(id -g)" \
			"${container_image}" \
			bash -c "make PREFIX="${PREFIX}" DESTDIR="${DESTDIR}" ${EXTRA_OPTS} install"
		;;
esac

for vmm in ${VMM_CONFIGS}; do
	for config_file in "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-${vmm}"*.toml; do
		if [ -f "${config_file}" ]; then
			if [ ${ARCH} == "ppc64le" ]; then
				# On ppc64le, replace image line with initrd line
				sed -i -e 's|^image = .*|initrd = "'${PREFIX}'/share/kata-containers/kata-containers-initrd.img"|' "${config_file}"
			fi
		fi
	done
done

pushd "${DESTDIR}/${PREFIX}/share/defaults/kata-containers"
	ln -sf "configuration-qemu.toml" configuration.toml
popd
