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

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="${SHIM_V2_CONTAINER_BUILDER:-$(get_shim_v2_image_name)}"

MEASURED_ROOTFS=${MEASURED_ROOTFS:-"no"}
DM_VERITY_FORMAT=${DM_VERITY_FORMAT:-"veritysetup"}
EXTRA_OPTS="${EXTRA_OPTS:-""}"

[ "${CROSS_BUILD}" == "true" ] && container_image_bk="${container_image}" && container_image="${container_image}-cross-build"
if [ "${MEASURED_ROOTFS}" == "yes" ]; then
	# TODO: also support measured rootfs without offloading container
	# image management to kata-agent.
	EXTRA_OPTS+=" DEFSERVICEOFFLOAD=true"

	info "Enable rootfs measurement config"
	root_hash_file="${repo_root_dir}/tools/osbuilder/root_hash.txt"
	[ -f "$root_hash_file" ] || \
		die "Root hash file for measured rootfs not found at ${root_hash_file}"

	root_hash=$(sudo sed -e 's/Root hash:\s*//g;t;d' "${root_hash_file}")
	case "${DM_VERITY_FORMAT}" in
		veritysetup)
			# Partition format compatible with "veritysetup open" but not with kernel's
			# "dm-mod.create" command line parameter. Add the ROOTMEASURECONFIG option
			# used by the Kata initramfs.
			root_measure_config="rootfs_verity.scheme=dm-verity rootfs_verity.hash=${root_hash}"
			EXTRA_OPTS+=" ROOTMEASURECONFIG=\"${root_measure_config}\""
			;;
		kernelinit)
			# Partition format compatible with kernel's "dm-mod.create" command line
			# option but not with "veritysetup open".
			salt=$(sudo sed -e 's/Salt:\s*//g;t;d' "${root_hash_file}")
			data_blocks=$(sudo sed -e 's/Data blocks:\s*//g;t;d' "${root_hash_file}")
			data_block_size=$(sudo sed -e 's/Data block size:\s*//g;t;d' "${root_hash_file}")
			data_sectors_per_block=$((data_block_size / 512))
			data_sectors=$((data_blocks * data_sectors_per_block))
			hash_block_size=$(sudo sed -e 's/Hash block size:\s*//g;t;d' "${root_hash_file}")
			EXTRA_OPTS+=" ROOTMEASURECONFIG=\"dm-mod.create=\\\"dm-verity,,,ro,0 ${data_sectors} verity 1 @ROOTFS_DEVICE@ @VERITY_DEVICE@ ${data_block_size} ${hash_block_size} ${data_blocks} 0 sha256 ${root_hash} ${salt}\\\"\""
			;;
		*)
			error "DM_VERITY_FORMAT(${DM_VERITY_FORMAT}) is incorrect (must be veritysetup or kernelinit)"
			return 1
			;;
	esac
fi

sudo docker pull ${container_image} || \
	(sudo docker ${BUILDX} build ${PLATFORM}  \
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

#Build rust project using cross build musl image to speed up
[[ "${CROSS_BUILD}" == "true" && ${ARCH} != "s390x" ]] && container_image="messense/rust-musl-cross:${GCC_ARCH}-musl" && CC=${GCC_ARCH}-unknown-linux-musl-gcc

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	--env CROSS_BUILD=${CROSS_BUILD} \
	--env ARCH=${ARCH} \
	--env CC="${CC}" \
	-w "${repo_root_dir}/src/runtime-rs" \
	"${container_image}" \
	bash -c "git config --global --add safe.directory ${repo_root_dir} && \
		make clean-generated-files && make PREFIX=${PREFIX} QEMUCMD=qemu-system-${arch}"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	--env CROSS_BUILD=${CROSS_BUILD} \
        --env ARCH=${ARCH} \
        --env CC="${CC}" \
	-w "${repo_root_dir}/src/runtime-rs" \
	"${container_image}" \
	bash -c "git config --global --add safe.directory ${repo_root_dir} && make PREFIX="${PREFIX}" DESTDIR="${DESTDIR}" install"

[ "${CROSS_BUILD}" == "true" ] && container_image="${container_image_bk}-cross-build"

# EXTRA_OPTS might contain kernel options similar to: dm-mod.create=\"dm-verity,,,ro,0 ...\"
# That quoted value format must be propagated without changes into the runtime config file.
# But that format gets modified:
#
# 1. When expanding EXTRA_OPTS as parameter to the make command below.
# 2. When shim's Makefile uses sed to substitute @KERNELPARAMS@ in the config file.
#
# Therefore, replace here any \" substring to compensate for the two changes described above.
EXTRA_OPTS_ESCAPED=$(echo "${EXTRA_OPTS}" | sed -e 's|\\\"|\\\\\\\\\\\\\\\\\\\\\\\"|g')

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}/src/runtime" \
	"${container_image}" \
	bash -c "git config --global --add safe.directory ${repo_root_dir} && \
		make clean-generated-files && make PREFIX=${PREFIX} QEMUCMD=qemu-system-${arch} ${EXTRA_OPTS_ESCAPED}"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}/src/runtime" \
	"${container_image}" \
	bash -c "git config --global --add safe.directory ${repo_root_dir} && make PREFIX="${PREFIX}" DESTDIR="${DESTDIR}" ${EXTRA_OPTS_ESCAPED} install"

for vmm in ${VMM_CONFIGS}; do
	config_file="${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-${vmm}.toml"
	if [ -f ${config_file} ]; then
		if [ ${ARCH} == "ppc64le" ]; then
 			sudo sed -i -e '/^image =/d' ${config_file}
 			sudo sed -i 's/^# \(initrd =.*\)/\1/g' ${config_file}
 		else
 			sudo sed -i -e '/^initrd =/d' ${config_file}
 		fi
	fi
done

pushd "${DESTDIR}/${PREFIX}/share/defaults/kata-containers"
	sudo ln -sf "configuration-qemu.toml" configuration.toml
popd
