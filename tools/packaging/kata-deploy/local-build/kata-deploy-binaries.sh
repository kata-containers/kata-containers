#!/usr/bin/env bash
# Copyright (c) 2018-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

readonly project="kata-containers"

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

source "${script_dir}/../../scripts/lib.sh"

readonly prefix="/opt/kata"
readonly static_build_dir="${repo_root_dir}/tools/packaging/static-build"
readonly version_file="${repo_root_dir}/VERSION"
readonly versions_yaml="${repo_root_dir}/versions.yaml"

readonly busybox_builder="${static_build_dir}/busybox/build.sh"
readonly agent_builder="${static_build_dir}/agent/build.sh"
readonly coco_guest_components_builder="${static_build_dir}/coco-guest-components/build.sh"
readonly clh_builder="${static_build_dir}/cloud-hypervisor/build-static-clh.sh"
readonly firecracker_builder="${static_build_dir}/firecracker/build-static-firecracker.sh"
readonly kernel_builder="${static_build_dir}/kernel/build.sh"
readonly ovmf_builder="${static_build_dir}/ovmf/build.sh"
readonly pause_image_builder="${static_build_dir}/pause-image/build.sh"
readonly qemu_builder="${static_build_dir}/qemu/build-static-qemu.sh"
readonly qemu_experimental_builder="${static_build_dir}/qemu/build-static-qemu-experimental.sh"
readonly stratovirt_builder="${static_build_dir}/stratovirt/build-static-stratovirt.sh"
readonly shimv2_builder="${static_build_dir}/shim-v2/build.sh"
readonly virtiofsd_builder="${static_build_dir}/virtiofsd/build.sh"
readonly nydus_builder="${static_build_dir}/nydus/build.sh"
readonly rootfs_builder="${repo_root_dir}/tools/packaging/guest-image/build_image.sh"
readonly tools_builder="${static_build_dir}/tools/build.sh"
readonly se_image_builder="${repo_root_dir}/tools/packaging/guest-image/build_se_image.sh"

ARCH=${ARCH:-$(uname -m)}
BUSYBOX_CONF_FILE="${BUSYBOX_CONF_FILE:-}"
MEASURED_ROOTFS=${MEASURED_ROOTFS:-no}
PULL_TYPE=${PULL_TYPE:-default}
USE_CACHE="${USE_CACHE:-"yes"}"
ARTEFACT_REGISTRY="${ARTEFACT_REGISTRY:-ghcr.io}"
ARTEFACT_REPOSITORY="${ARTEFACT_REPOSITORY:-kata-containers}"
ARTEFACT_REGISTRY_USERNAME="${ARTEFACT_REGISTRY_USERNAME:-}"
ARTEFACT_REGISTRY_PASSWORD="${ARTEFACT_REGISTRY_PASSWORD:-}"
TARGET_BRANCH="${TARGET_BRANCH:-main}"
PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-}"
KERNEL_HEADERS_PKG_TYPE="${KERNEL_HEADERS_PKG_TYPE:-deb}"
RELEASE="${RELEASE:-"no"}"

workdir="${WORKDIR:-$PWD}"

destdir="${workdir}/kata-static"

default_binary_permissions='0744'

die() {
	msg="$*"
	echo "ERROR: ${msg}" >&2
	exit 1
}

info() {
	echo "INFO: $*"
}

error() {
	echo "ERROR: $*"
}

usage() {
	return_code=${1:-0}
	cat <<EOF
This script is used as part of the ${project} release process.
It is used to create a tarball with static binaries.


Usage:
${script_name} <options> [version]

Args:
version: The kata version that will be use to create the tarball

options:

-h|--help      	      : Show this help
-s             	      : Silent mode (produce output in case of failure only)
--build=<asset>       :
	all
	agent
	agent-ctl
	boot-image-se
	coco-guest-components
	cloud-hypervisor
	cloud-hypervisor-glibc
	csi-kata-directvolume
	firecracker
	genpolicy
	kata-ctl
	kata-manager
	kernel
	kernel-confidential
	kernel-dragonball-experimental
	kernel-experimental
	kernel-nvidia-gpu
	kernel-nvidia-gpu-confidential
	nydus
	pause-image
	ovmf
	ovmf-sev
	qemu
	qemu-snp-experimental
	stratovirt
	rootfs-image
	rootfs-image-confidential
	rootfs-image-mariner
	rootfs-initrd
	rootfs-initrd-confidential
	runk
	shim-v2
	trace-forwarder
	virtiofsd
EOF

	exit "${return_code}"
}

get_kernel_headers_dir() {
	local kernel_name"=${1:-}"
	[ -z "${kernel_name}" ] && die "kernel name is a required argument"

	local kernel_headers_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/${kernel_name}/builddir"

	echo "${kernel_headers_dir}"
}

get_kernel_modules_dir() {
	local kernel_version="${1:-}"
	local kernel_kata_config_version="${2:-}"
	local kernel_name"=${3:-}"
	[ -z "${kernel_version}" ] && die "kernel version is a required argument"
	[ -z "${kernel_kata_config_version}" ] && die "kernel kata config version is a required argument"
	[ -z "${kernel_name}" ] && die "kernel name is a required argument"

	local version=${kernel_version#v}
	local numeric_final_version=${version}

	# Every first release of a kernel is x.y, while the resulting folder would be x.y.0
	local rc=$(echo ${version} | grep -oE "\-rc[0-9]+$")
	if [ -n "${rc}" ]; then
		numeric_final_version="${numeric_final_version%"${rc}"}"
	fi

	local dots=$(echo ${version} | grep -o '\.' | wc -l)
	[ "${dots}" == "1" ] && numeric_final_version="${numeric_final_version}.0"

	if [ -n "${rc}" ]; then
		numeric_final_version="${numeric_final_version}${rc}"
	fi

	local kernel_modules_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/${kernel_name}/builddir/kata-linux-${version}-${kernel_kata_config_version}/lib/modules/${numeric_final_version}"
	case ${kernel_name} in
		kernel-nvidia-gpu-confidential)
			kernel_modules_dir+="-nvidia-gpu-confidential"
			;;
		*)
			;;
	esac

	echo ${kernel_modules_dir}
}

cleanup_and_fail_shim_v2_specifics() {
	rm -f "${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/shim-v2-root_hash.txt"

	return $(cleanup_and_fail "${1:-}" "${2:-}")
}

cleanup_and_fail() {
	local component_tarball_name="${1:-}"
	local extra_tarballs="${2:-}"

	rm -f "${component_tarball_name}"

	if [ -n "${extra_tarballs}" ]; then
		local mapping
		IFS=' ' read -a mapping <<< "${extra_tarballs}"
		for m in ${mapping[@]}; do
			local extra_tarball_name=${m%:*}
			rm -f "${extra_tarball_name}"
		done
	fi

	return 1
}

install_cached_shim_v2_tarball_get_root_hash() {
	if [ "${MEASURED_ROOTFS}" != "yes" ]; then
		return 0
	fi

	local tarball_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	local image_conf_tarball="kata-static-rootfs-image-confidential.tar.xz"

	local root_hash_basedir="./opt/kata/share/kata-containers/"

	tar xvf "${tarball_dir}/${image_conf_tarball}" ${root_hash_basedir}root_hash.txt --transform s,${root_hash_basedir},,
	mv root_hash.txt "${tarball_dir}/root_hash.txt"

	return 0
}

install_cached_shim_v2_tarball_compare_root_hashes() {
	if [ "${MEASURED_ROOTFS}" != "yes" ]; then
		return 0
	fi

	local tarball_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"

	[ -f shim-v2-root_hash.txt ] || return 1

	diff "${tarball_dir}/root_hash.txt" shim-v2-root_hash.txt || return 1

	return 0
}

install_cached_tarball_component() {
	if [ "${USE_CACHE}" != "yes" ]; then
		return 1
	fi

	local component="${1}"
	local current_version="${2}-$(git log -1 --abbrev=9 --pretty=format:"%h" ${repo_root_dir}/tools/packaging/kata-deploy/local-build)"
	local current_image_version="${3}"
	local component_tarball_name="${4}"
	local component_tarball_path="${5}"
	# extra_tarballs must be in the following format:
	# "tarball1_name:tarball1_path tarball2_name:tarball2_path ... tarballN_name:tarballN_path"
	local extra_tarballs="${6:-}"

	if [ "${component}" = "shim-v2" ]; then
		install_cached_shim_v2_tarball_get_root_hash
	fi

	oras pull ${ARTEFACT_REGISTRY}/${ARTEFACT_REPOSITORY}/cached-artefacts/${build_target}:latest-${TARGET_BRANCH}-$(uname -m) || return 1

	cached_version="$(cat ${component}-version)"
	cached_image_version="$(cat ${component}-builder-image-version)"

	rm -f ${component}-version
	rm -f ${component}-builder-image-version

	[ "${cached_image_version}" != "${current_image_version}" ] && return $(cleanup_and_fail "${component_tarball_path}" "${extra_tarballs}")
	[ "${cached_version}" != "${current_version}" ] && return $(cleanup_and_fail "${component_tarball_path}" "${extra_tarballs}")
	sha256sum -c "${component}-sha256sum" || return $(cleanup_and_fail "${component_tarball_path}" "${extra_tarballs}")

	if [ "${component}" = "shim-v2" ]; then
		install_cached_shim_v2_tarball_compare_root_hashes || return $(cleanup_and_fail_shim_v2_specifics "${component_tarball_path}" "${extra_tarballs}")
	fi

	info "Using cached tarball of ${component}"
	mv "${component_tarball_name}" "${component_tarball_path}"

	[ -z "${extra_tarballs}" ] && return 0

	local mapping
	IFS=' ' read -a mapping <<< "${extra_tarballs}"
	for m in ${mapping[@]}; do
		local extra_tarball_name=${m%:*}
		local extra_tarball_path=${m#*:}

		mv ${extra_tarball_name} ${extra_tarball_path}
	done
}

get_agent_tarball_path() {
	agent_local_build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	agent_tarball_name="kata-static-agent.tar.xz"

	echo "${agent_local_build_dir}/${agent_tarball_name}"
}

get_coco_guest_components_tarball_path() {
	coco_guest_components_local_build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	coco_guest_components_tarball_name="kata-static-coco-guest-components.tar.xz"

	echo "${coco_guest_components_local_build_dir}/${coco_guest_components_tarball_name}"
}

get_latest_coco_guest_components_artefact_and_builder_image_version() {
	local coco_guest_components_version=$(get_from_kata_deps ".externals.coco-guest-components.version")
	local coco_guest_components_toolchain=$(get_from_kata_deps ".externals.coco-guest-components.toolchain")
	local latest_coco_guest_components_artefact="${coco_guest_components_version}-${coco_guest_components_toolchain}"
	local latest_coco_guest_components_builder_image="$(get_coco_guest_components_image_name)"

	echo "${latest_coco_guest_components_artefact}-${latest_coco_guest_components_builder_image}"
}

get_pause_image_tarball_path() {
	pause_image_local_build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	pause_image_tarball_name="kata-static-pause-image.tar.xz"

	echo "${pause_image_local_build_dir}/${pause_image_tarball_name}"
}

get_latest_pause_image_artefact_and_builder_image_version() {
	local pause_image_repo="$(get_from_kata_deps ".externals.pause.repo")"
	local pause_image_version=$(get_from_kata_deps ".externals.pause.version")
	local latest_pause_image_artefact="${pause_image_repo}-${pause_image_version}"
	local latest_pause_image_builder_image="$(get_pause_image_name)"

	echo "${latest_pause_image_artefact}-${latest_pause_image_builder_image}"
}

get_latest_kernel_confidential_artefact_and_builder_image_version() {
		local kernel_version=$(get_from_kata_deps ".assets.kernel.confidential.version")
		local kernel_kata_config_version="$(cat ${repo_root_dir}/tools/packaging/kernel/kata_config_version)"
		local latest_kernel_artefact="${kernel_version}-${kernel_kata_config_version}-$(get_last_modification $(dirname $kernel_builder))"
		local latest_kernel_builder_image="$(get_kernel_image_name)"

		echo "${latest_kernel_artefact}-${latest_kernel_builder_image}"
}

#Install guest image
install_image() {
	local variant="${1:-}"

	image_type="image"
	os_name="$(get_from_kata_deps ".assets.image.architecture.${ARCH}.name")"
	os_version="$(get_from_kata_deps ".assets.image.architecture.${ARCH}.version")"
	if [ -n "${variant}" ]; then
		image_type+="-${variant}"
		os_name="$(get_from_kata_deps ".assets.image.architecture.${ARCH}.${variant}.name")"
		os_version="$(get_from_kata_deps ".assets.image.architecture.${ARCH}.${variant}.version")"
	fi

	local component="rootfs-${image_type}"

	local osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version="$(get_from_kata_deps ".externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps ".externals.libseccomp.version")"
	local rust_version="$(get_from_kata_deps ".languages.rust.meta.newest-version")"
	local agent_last_commit=$(merge_two_hashes \
		"$(get_last_modification "${repo_root_dir}/src/agent")" \
		"$(get_last_modification "${repo_root_dir}/tools/packaging/static-build/agent")")


	latest_artefact="$(get_kata_version)-${os_name}-${os_version}-${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${image_type}"
	if [ "${variant}" == "confidential" ]; then
		# For the confidential image we depend on the kernel built in order to ensure that
		# measured boot is used
		latest_artefact+="-$(get_latest_kernel_confidential_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_coco_guest_components_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_pause_image_artefact_and_builder_image_version)"
	fi

	latest_builder_image=""

	install_cached_tarball_component \
		"${component}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "Create image"

	if [ -n "${variant}" ]; then
		if [[ "${variant}" == *confidential ]]; then
			export COCO_GUEST_COMPONENTS_TARBALL="$(get_coco_guest_components_tarball_path)"
			export PAUSE_IMAGE_TARBALL="$(get_pause_image_tarball_path)"
		fi
	fi

	export AGENT_TARBALL=$(get_agent_tarball_path)
	export AGENT_POLICY=yes

	"${rootfs_builder}" --osname="${os_name}" --osversion="${os_version}" --imagetype=image --prefix="${prefix}" --destdir="${destdir}" --image_initrd_suffix="${variant}"
}

#Install guest image for confidential guests
install_image_confidential() {
	if [ "${ARCH}" == "s390x" ]; then
		export MEASURED_ROOTFS=no
	else
		export MEASURED_ROOTFS=yes
	fi
	export PULL_TYPE=default
	install_image "confidential"
}

#Install cbl-mariner guest image
install_image_mariner() {
	install_image "mariner"
}

#Install guest initrd
install_initrd() {
	local variant="${1:-}"

	initrd_type="initrd"
	os_name="$(get_from_kata_deps ".assets.initrd.architecture.${ARCH}.name")"
	os_version="$(get_from_kata_deps ".assets.initrd.architecture.${ARCH}.version")"
	if [ -n "${variant}" ]; then
		initrd_type+="-${variant}"
		os_name="$(get_from_kata_deps ".assets.initrd.architecture.${ARCH}.${variant}.name")"
		os_version="$(get_from_kata_deps ".assets.initrd.architecture.${ARCH}.${variant}.version")"
	fi

	local component="rootfs-${initrd_type}"

	local osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version="$(get_from_kata_deps ".externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps ".externals.libseccomp.version")"
	local rust_version="$(get_from_kata_deps ".languages.rust.meta.newest-version")"
	local agent_last_commit=$(merge_two_hashes \
		"$(get_last_modification "${repo_root_dir}/src/agent")" \
		"$(get_last_modification "${repo_root_dir}/tools/packaging/static-build/agent")")

	latest_artefact="$(get_kata_version)-${os_name}-${os_version}-${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${initrd_type}"
	if [ "${variant}" == "confidential" ]; then
		# For the confidential initrd we depend on the kernel built in order to ensure that
		# measured boot is used
		latest_artefact+="-$(get_latest_kernel_confidential_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_coco_guest_components_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_pause_image_artefact_and_builder_image_version)"
	fi

	latest_builder_image=""

	[[ "${ARCH}" == "aarch64" && "${CROSS_BUILD}" == "true" ]] && echo "warning: Don't cross build initrd for aarch64 as it's too slow" && exit 0

	install_cached_tarball_component \
		"${component}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "Create initrd"

	if [ -n "${variant}" ]; then
		if [[ "${variant}" == *confidential ]]; then
			export COCO_GUEST_COMPONENTS_TARBALL="$(get_coco_guest_components_tarball_path)"
			export PAUSE_IMAGE_TARBALL="$(get_pause_image_tarball_path)"
		fi
	else
		# No variant is passed, it means vanilla kata containers
		if [ "${os_name}" = "alpine" ]; then
			export AGENT_INIT=yes
		fi
	fi

	export AGENT_TARBALL=$(get_agent_tarball_path)
	export AGENT_POLICY=yes

	"${rootfs_builder}" --osname="${os_name}" --osversion="${os_version}" --imagetype=initrd --prefix="${prefix}" --destdir="${destdir}" --image_initrd_suffix="${variant}"
}

#Install guest initrd for confidential guests
install_initrd_confidential() {
	if [ "${ARCH}" == "s390x" ]; then
		export MEASURED_ROOTFS=no
	else
		export MEASURED_ROOTFS=yes
	fi
	export PULL_TYPE=default
	install_initrd "confidential"
}

# For all nvidia_gpu targets we can customize the stack that is enbled
# in the VM by setting the NVIDIA_GPU_STACK= environment variable
#
# latest | lts | version
#              -> use the latest and greatest driver,
#                 lts release or e.g. version=550.127.1
# driver       -> enable open or closed drivers
# debug        -> enable debugging support
# compute      -> enable the compute GPU stack, includes utility
# graphics     -> enable the graphics GPU stack, includes compute
# dcgm         -> enable the DCGM stack + DGCM exporter
# nvswitch     -> enable DGX like systems
# gpudirect    -> enable use-cases like GPUDirect RDMA, GPUDirect GDS
# dragonball   -> enable dragonball support
#
# The full stack can be enabled by setting all the options like:
#
# NVIDIA_GPU_STACK="latest,compute,dcgm,nvswitch,gpudirect"
#
# Install NVIDIA GPU image
install_image_nvidia_gpu() {
	export AGENT_POLICY="yes"
	export EXTRA_PKGS="apt"
	NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:-"latest,compute,dcgm"}
	install_image "nvidia-gpu"
}

# Install NVIDIA GPU initrd
install_initrd_nvidia_gpu() {
	export AGENT_POLICY="yes"
	export EXTRA_PKGS="apt"
	NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:-"latest,compute,dcgm"}
	install_initrd "nvidia-gpu"
}

# Instal NVIDIA GPU confidential image
install_image_nvidia_gpu_confidential() {
	export AGENT_POLICY="yes"
	export EXTRA_PKGS="apt"
	# TODO: export MEASURED_ROOTFS=yes
	NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:-"latest,compute"}
	install_image "nvidia-gpu-confidential"
}

# Install NVIDIA GPU confidential initrd
install_initrd_nvidia_gpu_confidential() {
	export AGENT_POLICY="yes"
	export EXTRA_PKGS="apt"
	# TODO: export MEASURED_ROOTFS=yes
	NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:-"latest,compute"}
	install_initrd "nvidia-gpu-confidential"
}


install_se_image() {
	info "Create IBM SE image configured with AA_KBC=${AA_KBC}"
	"${se_image_builder}" --destdir="${destdir}"
}

#Install kernel component helper
install_cached_kernel_tarball_component() {
	local kernel_name=${1}
	local extra_tarballs="${2:-}"

	latest_artefact="${kernel_version}-${kernel_kata_config_version}-$(get_last_modification $(dirname $kernel_builder))"
	latest_builder_image="$(get_kernel_image_name)"

	install_cached_tarball_component \
		"${kernel_name}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		"${extra_tarballs}" \
		|| return 1

	case ${kernel_name} in
		"kernel-nvidia-gpu"*"")
			local kernel_headers_dir=$(get_kernel_headers_dir "${kernel_name}")
			mkdir -p ${kernel_headers_dir} || true
			tar xvf ${workdir}/${kernel_name}/builddir/kata-static-${kernel_name}-headers.tar.xz -C "${kernel_headers_dir}" || return 1
			;;& # fallthrough in the confidential case we need the modules.tar.xz and for every kernel-nvidia-gpu we need the headers
		"kernel"*"-confidential")
			local modules_dir=$(get_kernel_modules_dir ${kernel_version} ${kernel_kata_config_version} ${build_target})
			mkdir -p "${modules_dir}" || true
			tar xvf "${workdir}/kata-static-${kernel_name}-modules.tar.xz" -C "${modules_dir}" || return 1
			;;
	esac

	return 0
}

#Install kernel asset
install_kernel_helper() {
	local kernel_yaml_path="${1}"
	local kernel_name="${2}"
	local extra_cmd="${3:-}"
	local extra_tarballs=""

	export kernel_version="$(get_from_kata_deps .${kernel_yaml_path}.version)"
	export kernel_url="$(get_from_kata_deps .${kernel_yaml_path}.url)"
	export kernel_kata_config_version="$(cat ${repo_root_dir}/tools/packaging/kernel/kata_config_version)"

	if [[ "${kernel_name}" == "kernel"*"-confidential" ]]; then
		kernel_version="$(get_from_kata_deps .assets.kernel.confidential.version)"
		kernel_url="$(get_from_kata_deps .assets.kernel.confidential.url)"
	fi

	if [[ "${kernel_name}" == "kernel"*"-confidential" ]]; then
		local kernel_modules_tarball_name="kata-static-${kernel_name}-modules.tar.xz"
		local kernel_modules_tarball_path="${workdir}/${kernel_modules_tarball_name}"
		extra_tarballs="${kernel_modules_tarball_name}:${kernel_modules_tarball_path}"
	fi

	if [[ "${kernel_name}" == "kernel-nvidia-gpu*" ]]; then
		local kernel_headers_tarball_name="kata-static-${kernel_name}-headers.tar.xz"
		local kernel_headers_tarball_path="${workdir}/${kernel_headers_tarball_name}"
		extra_tarballs+=" ${kernel_headers_tarball_name}:${kernel_headers_tarball_path}"
	fi

	default_patches_dir="${repo_root_dir}/tools/packaging/kernel/patches"

	install_cached_kernel_tarball_component ${kernel_name} ${extra_tarballs} && return 0

	info "build ${kernel_name}"
	info "Kernel version ${kernel_version}"
	DESTDIR="${destdir}" PREFIX="${prefix}" "${kernel_builder}" -v "${kernel_version}" -f -u "${kernel_url}" "${extra_cmd}"
}

#Install kernel asset
install_kernel() {
	install_kernel_helper \
		"assets.kernel" \
		"kernel" \
		""
}

install_kernel_confidential() {
	if [ "${ARCH}" == "s390x" ]; then
		export MEASURED_ROOTFS=no
	else
		export MEASURED_ROOTFS=yes
	fi

	install_kernel_helper \
		"assets.kernel.confidential" \
		"kernel-confidential" \
		"-x"
}

install_kernel_dragonball_experimental() {
	install_kernel_helper \
		"assets.kernel-dragonball-experimental" \
		"kernel-dragonball-experimental" \
		"-e -t dragonball"
}

install_kernel_nvidia_gpu_dragonball_experimental() {
	install_kernel_helper \
		"assets.kernel-dragonball-experimental" \
		"kernel-dragonball-experimental" \
		"-e -t dragonball -g nvidia -H deb"
}

#Install GPU enabled kernel asset
install_kernel_nvidia_gpu() {
	install_kernel_helper \
		"assets.kernel" \
		"kernel-nvidia-gpu" \
		"-g nvidia -H deb"
}

#Install GPU and TEE enabled kernel asset
install_kernel_nvidia_gpu_confidential() {
	install_kernel_helper \
		"assets.kernel.confidential" \
		"kernel-nvidia-gpu-confidential" \
		"-x -g nvidia -H deb"
}

install_qemu_helper() {
	local qemu_repo_yaml_path="${1}"
	local qemu_version_yaml_path="${2}"
	local qemu_name="${3}"
	local builder="${4}"
	local qemu_tarball_name="${qemu_tarball_name:-kata-static-qemu.tar.gz}"

	export qemu_repo="$(get_from_kata_deps .${qemu_repo_yaml_path})"
	export qemu_version="$(get_from_kata_deps .${qemu_version_yaml_path})"

	latest_artefact="${qemu_version}-$(calc_qemu_files_sha256sum)"
	latest_builder_image="$(get_qemu_image_name)"

	install_cached_tarball_component \
		"${qemu_name}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static ${qemu_name}"
	"${builder}"
	tar xvf "${qemu_tarball_name}" -C "${destdir}"
}

# Install static qemu asset
install_qemu() {
	install_qemu_helper \
		"assets.hypervisor.qemu.url" \
		"assets.hypervisor.qemu.version" \
		"qemu" \
		"${qemu_builder}"
}

install_qemu_snp_experimental() {
	export qemu_suffix="snp-experimental"
	export qemu_tarball_name="kata-static-qemu-${qemu_suffix}.tar.gz"

	install_qemu_helper \
		"assets.hypervisor.qemu-${qemu_suffix}.url" \
		"assets.hypervisor.qemu-${qemu_suffix}.tag" \
		"qemu-${qemu_suffix}" \
		"${qemu_experimental_builder}"
}

# Install static firecracker asset
install_firecracker() {
	local firecracker_version=$(get_from_kata_deps ".assets.hypervisor.firecracker.version")

	latest_artefact="${firecracker_version}"
	latest_builder_image=""

	install_cached_tarball_component \
		"firecracker" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static firecracker"
	"${firecracker_builder}"
	info "Install static firecracker"
	mkdir -p "${destdir}/opt/kata/bin/"
	install -D --mode "$default_binary_permissions" release-${firecracker_version}-${ARCH}/firecracker-${firecracker_version}-${ARCH} "${destdir}/opt/kata/bin/firecracker"
	install -D --mode "$default_binary_permissions" release-${firecracker_version}-${ARCH}/jailer-${firecracker_version}-${ARCH} "${destdir}/opt/kata/bin/jailer"
}

install_clh_helper() {
	libc="${1}"
	features="${2}"
	suffix="${3:-""}"

	latest_artefact="$(get_from_kata_deps ".assets.hypervisor.cloud_hypervisor.version")"
	latest_builder_image=""

	install_cached_tarball_component \
		"cloud-hypervisor${suffix}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static cloud-hypervisor"
	libc="${libc}" features="${features}" "${clh_builder}"
	info "Install static cloud-hypervisor"
	mkdir -p "${destdir}/opt/kata/bin/"
	install -D --mode "$default_binary_permissions" cloud-hypervisor/cloud-hypervisor "${destdir}/opt/kata/bin/cloud-hypervisor${suffix}"
}

# Install static cloud-hypervisor asset
install_clh() {
	if [[ "${ARCH}" == "x86_64" ]]; then
		features="mshv,tdx"
	else
		features=""
	fi

	install_clh_helper "musl" "${features}"
}

# Install static cloud-hypervisor-glibc asset
install_clh_glibc() {
	if [[ "${ARCH}" == "x86_64" ]]; then
		features="mshv"
	else
		features=""
	fi

	install_clh_helper "gnu" "${features}" "-glibc"
}

# Install static stratovirt asset
install_stratovirt() {
	local stratovirt_version=$(get_from_kata_deps ".assets.hypervisor.stratovirt.version")

	latest_artefact="${stratovirt_version}"
	latest_builder_image=""

	install_cached_tarball_component \
		"stratovirt" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static stratovirt"
	"${stratovirt_builder}"
	info "Install static stratovirt"
	mkdir -p "${destdir}/opt/kata/bin/"
	install -D --mode "$default_binary_permissions" static-stratovirt/stratovirt "${destdir}/opt/kata/bin/stratovirt"
}

# Install static virtiofsd asset
install_virtiofsd() {
	latest_artefact="$(get_from_kata_deps ".externals.virtiofsd.version")-$(get_from_kata_deps ".externals.virtiofsd.toolchain")"
	latest_builder_image="$(get_virtiofsd_image_name)"

	install_cached_tarball_component \
		"virtiofsd" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static virtiofsd"
	"${virtiofsd_builder}"
	info "Install static virtiofsd"
	mkdir -p "${destdir}/opt/kata/libexec/"
	install -D --mode "$default_binary_permissions" virtiofsd/virtiofsd "${destdir}/opt/kata/libexec/virtiofsd"
}

# Install static nydus asset
install_nydus() {
	[ "${ARCH}" == "aarch64" ] && ARCH=arm64

	latest_artefact="$(get_from_kata_deps ".externals.nydus.version")"
	latest_builder_image=""

	install_cached_tarball_component \
		"nydus" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static nydus"
	"${nydus_builder}"
	info "Install static nydus"
	mkdir -p "${destdir}/opt/kata/libexec/"
	ls -tl . || true
	ls -tl nydus-static || true
	install -D --mode "$default_binary_permissions" nydus-static/nydusd "${destdir}/opt/kata/libexec/nydusd"
}

#Install all components that are not assets
install_shimv2() {
	local shim_v2_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime")"
	local runtime_rs_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime-rs")"
	local protocols_last_commit="$(get_last_modification "${repo_root_dir}/src/libs/protocols")"
	local GO_VERSION="$(get_from_kata_deps ".languages.golang.meta.newest-version")"
	local RUST_VERSION="$(get_from_kata_deps ".languages.rust.meta.newest-version")"

	latest_artefact="$(get_kata_version)-${shim_v2_last_commit}-${protocols_last_commit}-${runtime_rs_last_commit}-${GO_VERSION}-${RUST_VERSION}"
	latest_builder_image="$(get_shim_v2_image_name)"

	install_cached_tarball_component \
		"shim-v2" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	export GO_VERSION
	export RUST_VERSION
	export MEASURED_ROOTFS
	if [ "${ARCH}" == "s390x" ]; then
		export MEASURED_ROOTFS=no
	fi

	DESTDIR="${destdir}" PREFIX="${prefix}" "${shimv2_builder}"
}

install_ovmf() {
	ovmf_type="${1:-x86_64}"
	tarball_name="${2:-edk2-x86_64.tar.gz}"

	local component_name="ovmf"
	[ "${ovmf_type}" == "sev" ] && component_name="ovmf-sev"

	latest_artefact="$(get_from_kata_deps ".externals.ovmf.${ovmf_type}.version")"
	latest_builder_image="$(get_ovmf_image_name)"

	install_cached_tarball_component \
		"${component_name}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	DESTDIR="${destdir}" PREFIX="${prefix}" ovmf_build="${ovmf_type}" "${ovmf_builder}"
	tar xvf "${builddir}/${tarball_name}" -C "${destdir}"
}

# Install OVMF SEV
install_ovmf_sev() {
	install_ovmf "sev" "edk2-sev.tar.gz"
}

install_busybox() {
	latest_artefact="$(get_from_kata_deps ".externals.busybox.version")"
	latest_builder_image="$(get_busybox_image_name)"

	install_cached_tarball_component \
		"${build_target}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static busybox"
	DESTDIR=${destdir} BUSYBOX_CONF_FILE=${BUSYBOX_CONF_FILE:?} "${busybox_builder}"
}

install_agent() {
	latest_artefact="$(get_kata_version)-$(git log -1 --abbrev=9 --pretty=format:"%h" ${repo_root_dir}/src/agent)"
	artefact_tag="$(git log -1 --pretty=format:"%H" ${repo_root_dir})"
	latest_builder_image="$(get_agent_image_name)"

	install_cached_tarball_component \
		"${build_target}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	export LIBSECCOMP_VERSION="$(get_from_kata_deps ".externals.libseccomp.version")"
	export LIBSECCOMP_URL="$(get_from_kata_deps ".externals.libseccomp.url")"
	export GPERF_VERSION="$(get_from_kata_deps ".externals.gperf.version")"
	export GPERF_URL="$(get_from_kata_deps ".externals.gperf.url")"

	info "build static agent"
	DESTDIR="${destdir}" AGENT_POLICY="yes" PULL_TYPE=${PULL_TYPE} "${agent_builder}"
}

install_coco_guest_components() {
	latest_artefact="$(get_from_kata_deps ".externals.coco-guest-components.version")-$(get_from_kata_deps ".externals.coco-guest-components.toolchain")"
	artefact_tag="$(get_from_kata_deps ".externals.coco-guest-components.version")"
	latest_builder_image="$(get_coco_guest_components_image_name)"

	install_cached_tarball_component \
		"${build_target}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static coco-guest-components"
	DESTDIR="${destdir}" "${coco_guest_components_builder}"
}

install_pause_image() {
	latest_artefact="$(get_from_kata_deps ".externals.pause.repo")-$(get_from_kata_deps ".externals.pause.version")"
	artefact_tag=${latest_artefact}
	latest_builder_image="$(get_pause_image_name)"

	install_cached_tarball_component \
		"${build_target}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static pause-image"
	DESTDIR="${destdir}" "${pause_image_builder}"
}


install_script_helper() {
	local script="${1:-}"
	[ -n "$script" ] || die "need script"

	local script_path

	# If the script isn't specified as an absolute or relative path,
	# find it.
	if grep -q '/' <<< "$script"
	then
		script_path="$script"
	else
		script_path=$(find "${repo_root_dir}/" -type f -name "$script")
	fi

	local script_file
	script_file=$(basename "$script_path")

	local script_file_name

	# Remove any extension
	script_file_name="${script_file%%.*}"

	info "installing utility script ${script}"

	local bin_dir
	bin_dir="${destdir}/opt/kata/bin/"

	mkdir -p "$bin_dir"

	install -D \
		--mode "${default_binary_permissions}" \
		"${script_path}" \
		"${bin_dir}/${script_file}"

	[ "$script_file" = "$script_file_name" ] && return 0

	pushd "$bin_dir" &>/dev/null

	# Create a sym-link with the extension removed
	ln -sf "$script_file" "$script_file_name"

	popd &>/dev/null
}

install_tools_helper() {
	tool=${1}

	latest_artefact="$(get_kata_version)-$(git log -1 --abbrev=9 --pretty=format:"%h" ${repo_root_dir}/src/tools/${tool})"
	latest_builder_image="$(get_tools_image_name)"

	install_cached_tarball_component \
		"${tool}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	export LIBSECCOMP_VERSION="$(get_from_kata_deps ".externals.libseccomp.version")"
	export LIBSECCOMP_URL="$(get_from_kata_deps ".externals.libseccomp.url")"
	export GPERF_VERSION="$(get_from_kata_deps ".externals.gperf.version")"
	export GPERF_URL="$(get_from_kata_deps ".externals.gperf.url")"

	info "build static ${tool}"
	${tools_builder} ${tool}

	tool_binary=${tool}
	[ ${tool} = "agent-ctl" ] && tool_binary="kata-agent-ctl"
	[ ${tool} = "csi-kata-directvolume" ] && tool_binary="directvolplugin"
	[ ${tool} = "trace-forwarder" ] && tool_binary="kata-trace-forwarder"
	binary=$(find ${repo_root_dir}/src/tools/${tool}/ -type f -name ${tool_binary})

	if [[ "${tool}" == "genpolicy" ]]; then
		defaults_path="${destdir}/opt/kata/share/defaults/kata-containers"
		mkdir -p "${defaults_path}"
		install -D --mode 0644 ${repo_root_dir}/src/tools/${tool}/rules.rego "${defaults_path}/rules.rego"
		install -D --mode 0644 ${repo_root_dir}/src/tools/${tool}/genpolicy-settings.json "${defaults_path}/genpolicy-settings.json"
		binary_permissions="0755"
	else
		binary_permissions="$default_binary_permissions"
	fi

	if [[ "${tool}" == "agent-ctl" ]]; then
		defaults_path="${destdir}/opt/kata/share/defaults/kata-containers/agent-ctl"
		mkdir -p "${defaults_path}"
		install -D --mode 0644 ${repo_root_dir}/src/tools/${tool}/template/oci_config.json "${defaults_path}/oci_config.json"
	fi

	info "Install static ${tool_binary}"
	mkdir -p "${destdir}/opt/kata/bin/"
	[ ${tool} = "csi-kata-directvolume" ] && tool_binary="csi-kata-directvolume"
	install -D --mode ${binary_permissions} ${binary} "${destdir}/opt/kata/bin/${tool_binary}"
}

install_agent_ctl() {
	install_tools_helper "agent-ctl"
}

install_genpolicy() {
	install_tools_helper "genpolicy"
}

install_csi_kata_directvolume() {
	install_tools_helper "csi-kata-directvolume"
}

install_kata_ctl() {
	install_tools_helper "kata-ctl"
}

install_kata_manager() {
	install_script_helper "kata-manager.sh"
}

install_runk() {
	install_tools_helper "runk"
}

install_trace_forwarder() {
	install_tools_helper "trace-forwarder"
}

get_kata_version() {
	local v
	v=$(cat "${version_file}")
	echo ${v}
}

handle_build() {
	info "DESTDIR ${destdir}"

	latest_artefact=""
	latest_builder_image=""

	local build_target
	build_target="$1"

	export final_tarball_path="${workdir}/kata-static-${build_target}.tar.xz"
	export final_tarball_name="$(basename ${final_tarball_path})"
	rm -f ${final_tarball_name}

	case "${build_target}" in
	all)
		install_agent_ctl
		install_clh
		install_firecracker
		install_image
		install_image_confidential
		install_initrd
		install_initrd_confidential
		install_initrd_mariner
		install_kata_ctl
		install_kata_manager
		install_kernel
		install_kernel_confidential
		install_kernel_dragonball_experimental
		install_log_parser_rs
		install_nydus
		install_ovmf
		install_ovmf_sev
		install_qemu
		install_qemu_snp_experimental
		install_stratovirt
		install_runk
		install_shimv2
		install_trace_forwarder
		install_virtiofsd
		;;

	agent) install_agent ;;

	agent-ctl) install_agent_ctl ;;

	busybox) install_busybox ;;

	boot-image-se) install_se_image ;;

	coco-guest-components) install_coco_guest_components ;;

	cloud-hypervisor) install_clh ;;

	cloud-hypervisor-glibc) install_clh_glibc ;;

	csi-kata-directvolume) install_csi_kata_directvolume ;;

	firecracker) install_firecracker ;;

	genpolicy) install_genpolicy ;;

	kata-ctl) install_kata_ctl ;;

	kata-manager) install_kata_manager ;;

	kernel) install_kernel ;;

	kernel-confidential) install_kernel_confidential ;;

	kernel-dragonball-experimental) install_kernel_dragonball_experimental ;;

	kernel-nvidia-gpu-dragonball-experimental) install_kernel_nvidia_gpu_dragonball_experimental ;;

	kernel-nvidia-gpu) install_kernel_nvidia_gpu ;;

	kernel-nvidia-gpu-confidential) install_kernel_nvidia_gpu_confidential ;;

	nydus) install_nydus ;;

	ovmf) install_ovmf ;;

	ovmf-sev) install_ovmf_sev ;;

	pause-image) install_pause_image ;;

	qemu) install_qemu ;;

	qemu-snp-experimental) install_qemu_snp_experimental ;;

	stratovirt) install_stratovirt ;;

	rootfs-image) install_image ;;

	rootfs-image-confidential) install_image_confidential ;;

	rootfs-image-mariner) install_image_mariner ;;

	rootfs-initrd) install_initrd ;;

	rootfs-initrd-confidential) install_initrd_confidential ;;

	rootfs-nvidia-gpu-image) install_image_nvidia_gpu ;;

	rootfs-nvidia-gpu-initrd) install_initrd_nvidia_gpu ;;

	rootfs-nvidia-gpu-confidential-image) install_image_nvidia_gpu_confidential ;;

	rootfs-nvidia-gpu-confidential-initrd) install_initrd_nvidia_gpu_confidential ;;

	runk) install_runk ;;

	shim-v2) install_shimv2 ;;

	trace-forwarder) install_trace_forwarder ;;

	virtiofsd) install_virtiofsd ;;

	dummy)
		tar cvfJ ${final_tarball_path} --files-from /dev/null
	       	;;

	*)
		die "Invalid build target ${build_target}"
		;;
	esac

	if [ ! -f "${final_tarball_path}" ]; then
		cd "${destdir}"
		tar cvfJ "${final_tarball_path}" "."
	fi
	tar tvf "${final_tarball_path}"

	case ${build_target} in
		kernel-nvidia-gpu*)
			local kernel_headers_final_tarball_path="${workdir}/kata-static-${build_target}-headers.tar.xz"
			if [ ! -f "${kernel_headers_final_tarball_path}" ]; then
				local kernel_headers_dir
				kernel_headers_dir=$(get_kernel_headers_dir "${build_target}")

				pushd "${kernel_headers_dir}"
				find . -type f -name "*.${KERNEL_HEADERS_PKG_TYPE}" -exec tar cvfJ "${kernel_headers_final_tarball_path}" {} +
				popd
			fi
			tar tvf "${kernel_headers_final_tarball_path}"
			;;& # fallthrough in the confidential case we need the modules.tar.xz and for every kernel-nvidia-gpu we need the headers

		kernel*-confidential)
			local modules_final_tarball_path="${workdir}/kata-static-${build_target}-modules.tar.xz"
			if [ ! -f "${modules_final_tarball_path}" ]; then
				local modules_dir=$(get_kernel_modules_dir ${kernel_version} ${kernel_kata_config_version} ${build_target})

				pushd "${modules_dir}"
				rm -f build
				tar cvfJ "${modules_final_tarball_path}" "."
				popd
			fi
			tar tvf "${modules_final_tarball_path}"
			;;
		shim-v2)
			if [ "${MEASURED_ROOTFS}" = "yes" ]; then
				local image_conf_tarball="${workdir}/kata-static-rootfs-image-confidential.tar.xz"
				if [ ! -f "${image_conf_tarball}" ]; then
					die "Building the shim-v2 with MEASURED_ROOTFS support requires a rootfs confidential image tarball"
				fi

				local root_hash_basedir="./opt/kata/share/kata-containers/"
				if ! tar xvf ${image_conf_tarball} ${root_hash_basedir}root_hash.txt --transform s,${root_hash_basedir},,; then
					die "Building the shim-v2 with MEASURED_ROOTFS support requres a rootfs confidential image tarball built with MEASURED_ROOTFS support"
				fi

				mv root_hash.txt ${workdir}/shim-v2-root_hash.txt
			fi
			;;
	esac

	pushd ${workdir}
	echo "${latest_artefact}-$(git log -1 --abbrev=9 --pretty=format:"%h" ${repo_root_dir}/tools/packaging/kata-deploy/local-build)" > ${build_target}-version
	echo "${latest_builder_image}" > ${build_target}-builder-image-version
	sha256sum "${final_tarball_name}" > ${build_target}-sha256sum

	if [ "${PUSH_TO_REGISTRY}" = "yes" ]; then
		if [ -z "${ARTEFACT_REGISTRY}" ] ||
			[ -z "${ARTEFACT_REPOSITORY}" ] ||
			[ -z "${ARTEFACT_REGISTRY_USERNAME}" ] ||
			[ -z "${ARTEFACT_REGISTRY_PASSWORD}" ] ||
		      	[ -z "${TARGET_BRANCH}" ]; then
			die "ARTEFACT_REGISTRY, ARTEFACT_REPOSITORY, ARTEFACT_REGISTRY_USERNAME, ARTEFACT_REGISTRY_PASSWORD and TARGET_BRANCH must be passed to the script when pushing the artefacts to the registry!"
		fi

		echo "${ARTEFACT_REGISTRY_PASSWORD}" | oras login "${ARTEFACT_REGISTRY}" -u "${ARTEFACT_REGISTRY_USERNAME}" --password-stdin

		tags=(latest-"${TARGET_BRANCH}")
		if [ -n "${artefact_tag:-}" ]; then
			tags+=("${artefact_tag}")
		fi
		if [ "${RELEASE}" == "yes" ]; then
			tags+=("$(cat "${version_file}")")
		fi

		echo "Pushing ${build_target} with tags: ${tags[*]}"

		normalized_tags=""
		for tag in "${tags[@]}"; do
			# tags can only contain lowercase and uppercase letters, digits, underscores, periods, and hyphens
			# and limited to 128 characters, so filter out non-printable characers, replace invalid printable
			# characters with underscode and trim down to leave enough space for the arch suffix
			tag_length_limit="$(expr 128 - $(echo "-$(uname -m)" | wc -c))"
			normalized_tag="$(echo "${tag}" \
				| tr -dc '[:print:]' \
				| tr -c '[a-zA-Z0-9\_\.\-]' _ \
				| head -c "${tag_length_limit}" \
			)-$(uname -m)"
			normalized_tags="${normalized_tags},${normalized_tag}"
		done
		declare -a files_to_push=(
			"${final_tarball_name}"
			"${build_target}-version"
			"${build_target}-builder-image-version"
			"${build_target}-sha256sum"
		)
		oci_image="${ARTEFACT_REGISTRY}/${ARTEFACT_REPOSITORY}/cached-artefacts/${build_target}:${normalized_tags}"
		case ${build_target} in
			kernel-nvidia-gpu)
				files_to_push+=(
					"kata-static-${build_target}-headers.tar.xz"
				)
				;;
			kernel-nvidia-gpu-confidential)
				files_to_push+=(
					"kata-static-${build_target}-modules.tar.xz"
					"kata-static-${build_target}-headers.tar.xz"
				)
				;;
			kernel*-confidential)
				files_to_push+=(
					"kata-static-${build_target}-modules.tar.xz"
				)
				;;
			shim-v2)
				if [ "${MEASURED_ROOTFS}" = "yes" ]; then
					files_to_push+=(
						"shim-v2-root_hash.txt"
					)
				fi
				;;
			*)
				;;
		esac
		oci_sha="$(oras push "${oci_image}" "${files_to_push[@]}" --format go-template='{{.reference}}' --no-tty)"
		echo "${oci_sha}" > "${build_target}-oci-image"
		oras logout "${ARTEFACT_REGISTRY}"
	fi

	popd
}

silent_mode_error_trap() {
	local stdout="$1"
	local stderr="$2"
	local t="$3"
	local log_file="$4"
	exec 1>&${stdout}
	exec 2>&${stderr}
	error "Failed to build: $t, logs:"
	cat "${log_file}"
	exit 1
}

main() {
	local build_targets
	local silent
	build_targets=(
		agent
		agent-ctl
		cloud-hypervisor
		coco-guest-components
		csi-kata-directvolume
		firecracker
		genpolicy
		kata-ctl
		kata-manager
		kernel
		kernel-experimental
		nydus
		pause-image
		qemu
		stratovirt
		rootfs-image
		rootfs-image-confidential
		rootfs-initrd
		rootfs-initrd-confidential
		rootfs-initrd-mariner
		runk
		shim-v2
		trace-forwarder
		virtiofsd
		dummy
	)
	silent=false
	while getopts "hs-:" opt; do
		case $opt in
		-)
			case "${OPTARG}" in
			build=*)
				build_targets=(${OPTARG#*=})
				;;
			help)
				usage 0
				;;
			*)
				usage 1
				;;
			esac
			;;
		h) usage 0 ;;
		s) silent=true ;;
		*) usage 1 ;;
		esac
	done
	shift $((OPTIND - 1))

	kata_version=$(get_kata_version)

	workdir="${workdir}/build"
	for t in "${build_targets[@]}"; do
		destdir="${workdir}/${t}/destdir"
		builddir="${workdir}/${t}/builddir"
		echo "Build kata version ${kata_version}: ${t}"
		mkdir -p "${destdir}"
		mkdir -p "${builddir}"
		if [ "${silent}" == true ]; then
			log_file="${builddir}/log"
			echo "build log: ${log_file}"
		fi
		(
			cd "${builddir}"
			if [ "${silent}" == true ]; then
				local stdout
				local stderr
				# Save stdout and stderr, to be restored
				# by silent_mode_error_trap() in case of
				# build failure.
				exec {stdout}>&1
				exec {stderr}>&2
				trap "silent_mode_error_trap $stdout $stderr $t \"$log_file\"" ERR
				handle_build "${t}" &>"$log_file"
			else
				handle_build "${t}"
			fi
		)
	done

}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
	main $@
fi
