#!/usr/bin/env bash
# Copyright (c) 2018-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[[ -z "${DEBUG}" ]] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

readonly project="kata-containers"

script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_name
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly script_dir

# shellcheck source=/dev/null
source "${script_dir}/../../scripts/lib.sh"

readonly prefix="/opt/kata"
# shellcheck disable=SC2154
readonly static_build_dir="${repo_root_dir}/tools/packaging/static-build"
readonly version_file="${repo_root_dir}/VERSION"
# shellcheck disable=SC2034
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
CONFIDENTIAL_GUEST=${CONFIDENTIAL_GUEST:-no}
USE_CACHE="${USE_CACHE:-"yes"}"
ARTEFACT_REGISTRY="${ARTEFACT_REGISTRY:-ghcr.io}"
ARTEFACT_REPOSITORY="${ARTEFACT_REPOSITORY:-kata-containers}"
ARTEFACT_REGISTRY_USERNAME="${ARTEFACT_REGISTRY_USERNAME:-}"
ARTEFACT_REGISTRY_PASSWORD="${ARTEFACT_REGISTRY_PASSWORD:-}"
GUEST_HOOKS_TARBALL_NAME="${GUEST_HOOKS_TARBALL_NAME:-}"
EXTRA_PKGS="${EXTRA_PKGS:-}"
REPO_URL="${REPO_URL:-}"
REPO_URL_X86_64="${REPO_URL_X86_64:-}"
REPO_COMPONENTS="${REPO_COMPONENTS:-}"
AGENT_POLICY="${AGENT_POLICY:-yes}"
TARGET_BRANCH="${TARGET_BRANCH:-main}"
PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-}"
RELEASE="${RELEASE:-"no"}"
KBUILD_SIGN_PIN="${KBUILD_SIGN_PIN:-}"
RUNTIME_CHOICE="${RUNTIME_CHOICE:-both}"
KERNEL_DEBUG_ENABLED=${KERNEL_DEBUG_ENABLED:-"no"}
INIT_DATA="${INIT_DATA:-yes}"

workdir="${WORKDIR:-${PWD}}"

destdir="${workdir}/kata-static"

default_binary_permissions='0744'

# Rootfs image variants that carry a dm-verity root hash (measured rootfs).
# Their hashes are collected at build time and consumed by the Rust runtime,
# so this list is walked in a few places - keep it in one spot to avoid drift.
readonly MEASURED_ROOTFS_VARIANTS=(
	base
	confidential
	coco-extension
	nvidia-gpu
	nvidia-gpu-confidential
	nvidia
	nvidia-gpu-extension
)

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

# Sanitize a string so it is valid as a Docker / OCI image tag component.
sanitize_tag_component() {
	echo "$1" | tr -dc '[:print:]' | tr -c 'a-zA-Z0-9_.\-' _
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
	firecracker
	genpolicy
	kata-ctl
	kata-manager
	kernel
	kernel-cca-confidential
	kernel-debug
	kernel-dragonball-experimental
	kernel-experimental
	kernel-nvidia-gpu
	nydus
	pause-image
	ovmf
	ovmf-sev
	ovmf-tdx
	ovmf-cca
	qemu
	qemu-cca-experimental
	qemu-snp-experimental
	qemu-tdx-experimental
	stratovirt
	rootfs-image
	rootfs-image-confidential
	rootfs-image-coco-extension
	rootfs-image-mariner
	rootfs-initrd
	rootfs-initrd-confidential
	shim-v2-go
	shim-v2-rust
	trace-forwarder
	virtiofsd
EOF

	exit "${return_code}"
}

get_kernel_modules_dir() {
	local kernel_version="${1:-}"
	local kernel_kata_config_version="${2:-}"
	local kernel_name="${3:-}"
	[[ -z "${kernel_version}" ]] && die "kernel version is a required argument"
	[[ -z "${kernel_kata_config_version}" ]] && die "kernel kata config version is a required argument"
	[[ -z "${kernel_name}" ]] && die "kernel name is a required argument"

	local version=${kernel_version#v}
	local numeric_final_version=${version}

	if [[ -z "${kernel_ref}" ]]; then
		local rc
		rc=$(echo "${version}" | grep -oE "\-rc[0-9]+$" || true)
		if [[ -n "${rc}" ]]; then
			numeric_final_version="${numeric_final_version%"${rc}"}"
		fi

		local dots
		dots=$(echo "${version}" | grep -o '\.' | wc -l) || true
		[[ "${dots}" == "1" ]] && numeric_final_version="${numeric_final_version}.0"

		if [[ -n "${rc}" ]]; then
			numeric_final_version="${numeric_final_version}${rc}"
		fi
	else
		# kernel_version should be vx.y.z-rcn-hash format when git is used
		numeric_final_version="${numeric_final_version%-*}+"
	fi

	local kernel_modules_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/${kernel_name}/builddir/kata-linux-${version}-${kernel_kata_config_version}/lib/modules/${numeric_final_version}"
	echo "${kernel_modules_dir}"
}

cleanup_and_fail_shim_v2_specifics() {
	local component="${1:-}"
	local component_tarball_path="${2:-}"
	local extra_tarballs="${3:-}"
	local tarball_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"

	for variant in "${MEASURED_ROOTFS_VARIANTS[@]}"; do
		local root_hash_file="${tarball_dir}/${component}-root_hash_${variant}.txt"
		[[ -f "${root_hash_file}" ]] && rm -f "${root_hash_file}"
	done

	cleanup_and_fail "${component_tarball_path}" "${extra_tarballs}"
}

cleanup_and_fail() {
	local component_tarball_name="${1:-}"
	local extra_tarballs="${2:-}"

	rm -f "${component_tarball_name}"

	if [[ -n "${extra_tarballs}" ]]; then
		local mapping
		IFS=' ' read -r -a mapping <<< "${extra_tarballs}"
		for m in "${mapping[@]}"; do
			local extra_tarball_name=${m%:*}
			rm -f "${extra_tarball_name}"
		done
	fi

	return 1
}

install_cached_shim_v2_tarball_get_root_hash() {
	local tarball_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	local root_hash_basedir="./opt/kata/share/kata-containers/"

	for variant in "${MEASURED_ROOTFS_VARIANTS[@]}"; do
		# The measured base image ships as kata-static-rootfs-image.tar.zst
		# (no variant suffix), but carries its root hash under the "base" label.
		local image_conf_tarball
		if [[ "${variant}" == "base" ]]; then
			image_conf_tarball="kata-static-rootfs-image.tar.zst"
		else
			image_conf_tarball="kata-static-rootfs-image-${variant}.tar.zst"
		fi
		local tarball_path="${tarball_dir}/${image_conf_tarball}"
		local root_hash_path="${root_hash_basedir}root_hash_${variant}.txt"

		# If variant does not exist we skip the current iteration.
		[[ ! -f "${tarball_path}" ]] && continue

		tar --zstd -tf "${tarball_path}" "${root_hash_path}" >/dev/null 2>&1 || continue
		tar --zstd -xvf "${tarball_path}" "${root_hash_path}" --transform s,"${root_hash_basedir}",, || die "Failed to extract root hash from ${tarball_path}"
		mv "root_hash_${variant}.txt" "${tarball_dir}/"
	done

	return 0
}

install_cached_shim_v2_tarball_compare_root_hashes() {
	local component="${1:-}"
	local found_any=""
	local tarball_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"

	for variant in "${MEASURED_ROOTFS_VARIANTS[@]}"; do
		local image_root_hash="${tarball_dir}/root_hash_${variant}.txt"
		local cached_root_hash="${component}-root_hash_${variant}.txt"

		# Skip if the current image tarball did not ship a root hash for this variant.
		[[ ! -f "${image_root_hash}" ]] && continue

		if [[ ! -f "${cached_root_hash}" ]] || ! cmp -s "${image_root_hash}" "${cached_root_hash}"; then
			info "Measured rootfs hash mismatch for ${component} variant ${variant}; rebuilding shim"
			return 1
		fi
		found_any="yes"
	done
	[[ -z "${found_any}" ]] && return 0

	return 0
}

install_cached_tarball_component() {
	if [[ "${USE_CACHE}" != "yes" ]]; then
		return 1
	fi

	local component="${1}"
	local current_version
	current_version="${2}-$(git log -1 --abbrev=9 --pretty=format:"%h" "${repo_root_dir}"/tools/packaging/kata-deploy/local-build)"
	local current_image_version="${3}"
	local component_tarball_name="${4}"
	local component_tarball_path="${5}"
	# extra_tarballs must be in the following format:
	# "tarball1_name:tarball1_path tarball2_name:tarball2_path ... tarballN_name:tarballN_path"
	local extra_tarballs="${6:-}"

	if [[ "${MEASURED_ROOTFS}" = "yes" ]] && \
		{ [[ "${component}" = "shim-v2-go" ]] || [[ "${component}" = "shim-v2-rust" ]]; }; then
		install_cached_shim_v2_tarball_get_root_hash
	fi

	oras pull "${ARTEFACT_REGISTRY}/${ARTEFACT_REPOSITORY}/cached-artefacts/${build_target}:latest-$(sanitize_tag_component "${TARGET_BRANCH}")-$(uname -m)" || return 1

	cached_version="$(cat "${component}"-version)"
	cached_image_version="$(cat "${component}"-builder-image-version)"

	rm -f "${component}"-version
	rm -f "${component}"-builder-image-version

	if [[ "${cached_image_version}" != "${current_image_version}" ]]; then
		cleanup_and_fail "${component_tarball_path}" "${extra_tarballs}"
		return 1
	fi
	if [[ "${cached_version}" != "${current_version}" ]]; then
		cleanup_and_fail "${component_tarball_path}" "${extra_tarballs}"
		return 1
	fi
	sha256sum -c "${component}-sha256sum" || { cleanup_and_fail "${component_tarball_path}" "${extra_tarballs}"; return 1; }

	if [[ "${MEASURED_ROOTFS}" = "yes" ]] && \
		{ [[ "${component}" = "shim-v2-go" ]] || [[ "${component}" = "shim-v2-rust" ]]; }; then
		install_cached_shim_v2_tarball_compare_root_hashes "${component}" || { cleanup_and_fail_shim_v2_specifics "${component}" "${component_tarball_path}" "${extra_tarballs}"; return 1; }
	fi

	info "Using cached tarball of ${component}"
	mv "${component_tarball_name}" "${component_tarball_path}"

	[[ -z "${extra_tarballs}" ]] && return 0

	local mapping
	IFS=' ' read -r -a mapping <<< "${extra_tarballs}"
	for m in "${mapping[@]}"; do
		local extra_tarball_name=${m%:*}
		local extra_tarball_path=${m#*:}

		mv "${extra_tarball_name}" "${extra_tarball_path}"
	done
}

get_agent_tarball_path() {
	agent_local_build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	agent_tarball_name="kata-static-agent.tar.zst"

	echo "${agent_local_build_dir}/${agent_tarball_name}"
}

get_coco_guest_components_tarball_path() {
	coco_guest_components_local_build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	coco_guest_components_tarball_name="kata-static-coco-guest-components.tar.zst"

	echo "${coco_guest_components_local_build_dir}/${coco_guest_components_tarball_name}"
}

get_latest_coco_guest_components_artefact_and_builder_image_version() {
	local coco_guest_components_version
	coco_guest_components_version=$(get_from_kata_deps ".externals.coco-guest-components.version")
	local coco_guest_components_toolchain
	coco_guest_components_toolchain=$(get_from_kata_deps ".externals.coco-guest-components.toolchain")
	local latest_coco_guest_components_artefact="${coco_guest_components_version}-${coco_guest_components_toolchain}"
	local latest_coco_guest_components_builder_image
	latest_coco_guest_components_builder_image="$(get_coco_guest_components_image_name)"

	echo "${latest_coco_guest_components_artefact}-${latest_coco_guest_components_builder_image}"
}

get_pause_image_tarball_path() {
	pause_image_local_build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	pause_image_tarball_name="kata-static-pause-image.tar.zst"

	echo "${pause_image_local_build_dir}/${pause_image_tarball_name}"
}

get_guest_hooks_tarball_path() {
	guest_hooks_local_build_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build"
	guest_hooks_tarball_name="${GUEST_HOOKS_TARBALL_NAME}"

	echo "${guest_hooks_local_build_dir}/${guest_hooks_tarball_name}"
}

get_latest_pause_image_artefact_and_builder_image_version() {
	local pause_image_repo
	pause_image_repo="$(get_from_kata_deps ".externals.pause.repo")"
	local pause_image_version
	pause_image_version=$(get_from_kata_deps ".externals.pause.version")
	local latest_pause_image_artefact="${pause_image_repo}-${pause_image_version}"
	local latest_pause_image_builder_image
	latest_pause_image_builder_image="$(get_pause_image_name)"

	echo "${latest_pause_image_artefact}-${latest_pause_image_builder_image}"
}

get_latest_kernel_artefact_and_builder_image_version() {
	local kernel_version
	local kernel_kata_config_version
	local latest_kernel_artefact
	local latest_kernel_builder_image

	kernel_version=$(get_from_kata_deps ".assets.kernel.version")
	kernel_kata_config_version="$(cat "${repo_root_dir}"/tools/packaging/kernel/kata_config_version)"
	latest_kernel_artefact="${kernel_version}-${kernel_kata_config_version}-$(get_last_modification "$(dirname "${kernel_builder}")")"
	latest_kernel_builder_image="$(get_kernel_image_name)"

	echo "${latest_kernel_artefact}-${latest_kernel_builder_image}"
}

get_latest_kernel_nvidia_artefact_and_builder_image_version() {
	local kernel_version
	local kernel_kata_config_version
	local latest_kernel_artefact
	local latest_kernel_builder_image

	kernel_version=$(get_from_kata_deps ".assets.kernel.nvidia.version")
	kernel_kata_config_version="$(cat "${repo_root_dir}"/tools/packaging/kernel/kata_config_version)"
	latest_kernel_artefact="${kernel_version}-${kernel_kata_config_version}-$(get_last_modification "$(dirname "${kernel_builder}")")"
	latest_kernel_builder_image="$(get_kernel_image_name)"

	echo "${latest_kernel_artefact}-${latest_kernel_builder_image}"
}

get_latest_nvidia_driver_version() {
	get_from_kata_deps ".externals.nvidia.driver.version"
}

get_latest_nvidia_ctk_version() {
	get_from_kata_deps ".externals.nvidia.ctk.version"
}

get_latest_nvidia_nvrc_version() {
	get_from_kata_deps ".externals.nvrc.version"
}

get_latest_nvidia_nvat_version() {
	get_from_kata_deps ".externals.nvidia.nvat.version"
}

#Install guest image
install_image() {
	local variant="${1:-}"

	image_type="image"
	os_name="$(get_from_kata_deps ".assets.image.architecture.${ARCH}.name")"
	os_version="$(get_from_kata_deps ".assets.image.architecture.${ARCH}.version")"
	if [[ -n "${variant}" ]]; then
		image_type+="-${variant}"
		os_name="$(get_from_kata_deps ".assets.image.architecture.${ARCH}.${variant}.name")"
		os_version="$(get_from_kata_deps ".assets.image.architecture.${ARCH}.${variant}.version")"
	fi

	local component="rootfs-${image_type}"

	local osbuilder_last_commit
	osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit
	guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local libs_last_commit
	libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version
	gperf_version="$(get_from_kata_deps ".externals.gperf.version")"
	local libseccomp_version
	libseccomp_version="$(get_from_kata_deps ".externals.libseccomp.version")"
	local rust_version
	rust_version="$(get_from_kata_deps ".languages.rust.meta.newest-version")"
	local agent_last_commit
	agent_last_commit=$(merge_two_hashes \
		"$(get_last_modification "${repo_root_dir}/src/agent")" \
		"$(get_last_modification "${repo_root_dir}/tools/packaging/static-build/agent")")


	latest_artefact="$(get_kata_version)-${os_name}-${os_version}-${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${image_type}"
	if [[ "${variant}" == *confidential ]]; then
		# For the confidential image we depend on the kernel built in order to ensure that
		# measured boot is used
		if [[ "${variant}" == "nvidia-gpu-confidential" ]]; then
			latest_artefact+="-$(get_latest_kernel_nvidia_artefact_and_builder_image_version)"
			latest_artefact+="-$(get_latest_nvidia_driver_version)"
			latest_artefact+="-$(get_latest_nvidia_ctk_version)"
			latest_artefact+="-$(get_latest_nvidia_nvrc_version)"
			latest_artefact+="-$(get_latest_nvidia_nvat_version)"
		else
			latest_artefact+="-$(get_latest_kernel_artefact_and_builder_image_version)"
		fi

		# Both the standard and NVIDIA confidential images bake the CoCo
		# guest components + pause image into the rootfs, so factor them
		# into the cache key.
		latest_artefact+="-$(get_latest_coco_guest_components_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_pause_image_artefact_and_builder_image_version)"
	fi

	if [[ "${variant}" == "nvidia-gpu" || "${variant}" == "nvidia-gpu-extension" ]]; then
		# If we bump the kernel we need to rebuild the image.  The gpu extension
		# carries the driver userspace carved out of the same chiseled tree,
		# so it is driver-versioned just like the monolith.
		latest_artefact+="-$(get_latest_kernel_nvidia_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_nvidia_driver_version)"
		latest_artefact+="-$(get_latest_nvidia_ctk_version)"
		latest_artefact+="-$(get_latest_nvidia_nvrc_version)"
	fi

	if [[ "${variant}" == "nvidia" ]]; then
		# The nvidia base image strips all driver userspace and resets the
		# kernel modules to in-tree only, so it is driver-agnostic: it depends
		# on the NVIDIA kernel build and NVRC (its init), but not on the driver
		# or container-toolkit versions.  That lets a single base image back
		# multiple driver-specific gpu extensions.
		latest_artefact+="-$(get_latest_kernel_nvidia_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_nvidia_nvrc_version)"
	fi

	# The base guest image (empty variant) is built as a measured rootfs so
	# that confidential configurations can dm-verity-protect it; non-confidential
	# configurations simply boot its data partition unverified.  Reflect the
	# measured build (and the kernel it is tied to, since measured boot depends
	# on it) in the cache key, and emit the root hash under a dedicated "base"
	# label so it never collides with the legacy confidential image hash.
	local root_hash_variant="${variant}"
	if [[ -z "${variant}" && "${MEASURED_ROOTFS:-no}" == "yes" ]]; then
		root_hash_variant="base"
		latest_artefact+="-measured-$(get_latest_kernel_artefact_and_builder_image_version)"
	fi
	export ROOT_HASH_VARIANT="${root_hash_variant}"

	latest_builder_image=""

	install_cached_tarball_component \
		"${component}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "Create image"

	if [[ -n "${variant}" ]]; then
		# Both the standard confidential image and the NVIDIA confidential
		# image bake the CoCo guest components + pause image into the
		# rootfs, so each stays a usable standalone monolithic CoCo image.
		# The runtime-rs split path instead ships these in the separate
		# CoCo extension image (rootfs-image-coco-extension).
		if [[ "${variant}" == *confidential ]]; then
			COCO_GUEST_COMPONENTS_TARBALL="$(get_coco_guest_components_tarball_path)"
			export COCO_GUEST_COMPONENTS_TARBALL
			PAUSE_IMAGE_TARBALL="$(get_pause_image_tarball_path)"
			export PAUSE_IMAGE_TARBALL
		fi
	fi

	AGENT_TARBALL=$(get_agent_tarball_path)
	export AGENT_TARBALL
	export AGENT_POLICY

	if [[ -n "${GUEST_HOOKS_TARBALL_NAME}" ]]; then
		GUEST_HOOKS_TARBALL="$(get_guest_hooks_tarball_path)"
		export GUEST_HOOKS_TARBALL
	fi

	if [[ -n "${EXTRA_PKGS}" ]]; then
		export EXTRA_PKGS
	fi

	if [[ -n "${REPO_URL}" ]]; then
		export REPO_URL
	fi

	if [[ -n "${REPO_URL_X86_64}" ]]; then
		export REPO_URL_X86_64
	fi

	if [[ -n "${REPO_COMPONENTS}" ]]; then
		export REPO_COMPONENTS
	fi

	"${rootfs_builder}" --osname="${os_name}" --osversion="${os_version}" --imagetype=image --prefix="${prefix}" --destdir="${destdir}" --image_initrd_suffix="${variant}"
}

#Install the base guest image
#
# The base image (kata-containers.img) is shared by both non-confidential and
# confidential configurations.  It is built once as a measured rootfs (dm-verity
# hash partition + root hash emitted under the "base" label).  Confidential
# configurations enforce that hash via the kernel command line, while
# non-confidential configurations ignore it and boot the data partition
# directly.  Measured rootfs is not used on s390x (Secure Execution measures the
# guest through a different mechanism), so the base stays unmeasured there.
install_image_base() {
	if [[ "${ARCH}" == "s390x" ]]; then
		export MEASURED_ROOTFS="no"
	else
		export MEASURED_ROOTFS="yes"
	fi
	install_image
}

#Install guest image for confidential guests
#
# During the transition to composable (base + extension) images this monolithic
# confidential image still bakes in the CoCo guest components and is the image
# used by the Go runtime. The runtime-rs shims instead use the base image plus
# the separately built CoCo extension image (rootfs-image-coco-extension),
# attached as an extra block device. Once the split path is validated for the
# Go runtime too, the components can stop being baked in here.
install_image_confidential() {
	export CONFIDENTIAL_GUEST="yes"
	if [[ "${ARCH}" == "s390x" ]]; then
		export MEASURED_ROOTFS="no"
	else
		export MEASURED_ROOTFS="yes"
	fi
	install_image "confidential"
}

#Install CoCo extension image (erofs+verity, contains CoCo guest components + pause)
install_image_coco_extension() {
	local component="rootfs-image-coco-extension"

	local coco_last_commit
	coco_last_commit="$(get_latest_coco_guest_components_artefact_and_builder_image_version)"
	local pause_last_commit
	pause_last_commit="$(get_latest_pause_image_artefact_and_builder_image_version)"

	latest_artefact="$(get_kata_version)-coco-extension-${coco_last_commit}-${pause_last_commit}"
	latest_builder_image=""

	install_cached_tarball_component \
		"${component}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "Create CoCo extension image"

	# Use a temp dir under the repo root so the path is valid both inside
	# the outer build-kata-deploy container and in the nested image-builder
	# container (Docker-in-Docker mounts use host paths).
	local extension_rootfs
	extension_rootfs="$(mktemp -d "${repo_root_dir}/.coco-extension-rootfs.XXXX")"

	COCO_GUEST_COMPONENTS_TARBALL="$(get_coco_guest_components_tarball_path)"
	PAUSE_IMAGE_TARBALL="$(get_pause_image_tarball_path)"

	info "Unpacking CoCo guest components into extension rootfs"
	tar --zstd -xvf "${COCO_GUEST_COMPONENTS_TARBALL}" -C "${extension_rootfs}"

	info "Unpacking pause image into extension rootfs"
	tar --zstd -xvf "${PAUSE_IMAGE_TARBALL}" -C "${extension_rootfs}"

	# Data-driven extension manifest consumed by kata-agent. It describes the
	# components shipped in this extension so the agent needs no per-bundle code
	# changes. All paths are relative to the extension mount point
	# (/run/kata-extensions/coco). The "${var}" tokens in the [[process]] entries are
	# substituted by kata-agent from its runtime context.
	info "Writing CoCo extension component manifest"
	local manifest_dir="${extension_rootfs}/etc/kata-extensions"
	mkdir -p "${manifest_dir}"
	cat > "${manifest_dir}/components.toml" <<'EOF'
schema_version = 1

[paths]
"ocicrypt-config" = "etc/ocicrypt_config.json"
"pause-bundle"    = "pause_bundle"

[[process]]
id            = "attestation-agent"
level         = 1
args          = ["--attestation_sock", "${aa_attestation_uri}"]
optional_args = [{ when = "initdata_toml_path", args = ["--initdata-toml", "${initdata_toml_path}"] }]
config        = "${aa_config_path}"
wait_socket   = "${aa_attestation_socket}"
timeout_secs  = "${launch_process_timeout}"
# The extension ships both the stock attestation-agent and the NVIDIA-attester
# build; the consumer selects one via the "attester_variant" context value
# (kata-agent uses "default", NVRC uses "nvidia").
select        = "${attester_variant}"

  [process.variants.default]
  path = "usr/local/bin/attestation-agent"

  [process.variants.nvidia]
  path = "usr/local/bin/attestation-agent-nv"
  # attestation-agent-nv links libnvat.so (bundled in this CoCo extension under
  # usr/local/lib), which dlopens libnvidia-ml.so.1 for GPU attestation evidence.
  # Only NVAT's own libs need LD_LIBRARY_PATH: NVML lives in the GPU extension,
  # which NVRC folds into the guest loader cache before starting kata-agent, so it
  # resolves without a path here (see NVRC gpu::setup).
  env  = { LD_LIBRARY_PATH = "${extension_root}/usr/local/lib" }

[[process]]
id           = "confidential-data-hub"
level        = 2
path         = "usr/local/bin/confidential-data-hub"
config       = "${cdh_config_path}"
# CDH's secure_mount shells out (by PATH lookup) to cryptsetup for encrypted
# storage and to mke2fs/mkfs.ext4/dd for the filesystem. cryptsetup is CoCo-only
# and ships in this extension under usr/sbin (see
# build-static-coco-guest-components.sh); the plain mkfs/dd tooling lives in the
# nvidia base image's /sbin and /bin. The agent launches CDH with
# PATH=/bin:/sbin:/usr/bin:/usr/sbin, but setting any env here overrides it
# wholesale, so prepend the extension's usr/sbin and restore the base dirs.
env          = { OCICRYPT_KEYPROVIDER_CONFIG = "${ocicrypt_config_path}", PATH = "${extension_root}/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin" }
wait_socket  = "${cdh_socket}"
timeout_secs = "${launch_process_timeout}"

[[process]]
id           = "api-server-rest"
level        = 3
path         = "usr/local/bin/api-server-rest"
args         = ["--features", "${rest_api_features}"]
timeout_secs = "0"
EOF

	local install_dir="${destdir}/${prefix}/share/kata-containers/"
	mkdir -p "${install_dir}"

	local image_builder="${repo_root_dir}/tools/osbuilder/image-builder/image_builder.sh"

	export USE_DOCKER="1"
	export BUILD_VARIANT="coco-extension"
	export FS_TYPE="erofs"
	# Mirror the base/confidential images: s390x does not use a measured rootfs
	# (Secure Execution measures the guest through a different mechanism), so the
	# extension carries no dm-verity hash there and is mounted off its raw
	# partition instead.
	if [[ "${ARCH}" == "s390x" ]]; then
		export MEASURED_ROOTFS="no"
	else
		export MEASURED_ROOTFS="yes"
	fi
	export SKIP_DAX_HEADER="yes"
	export SKIP_ROOTFS_CHECK="yes"

	"${image_builder}" -o "${install_dir}/kata-containers-coco-extension.img" "${extension_rootfs}"

	if [[ -e "${install_dir}/root_hash_coco-extension.txt" ]]; then
		info "Root hash file: ${install_dir}/root_hash_coco-extension.txt"
	fi

	rm -rf "${extension_rootfs}"
}

#Install cbl-mariner guest image
install_image_mariner() {
	export IMAGE_SIZE_ALIGNMENT_MB=2
	install_image "mariner"
}

#Install guest initrd
install_initrd() {
	local variant="${1:-}"

	initrd_type="initrd"
	os_name="$(get_from_kata_deps ".assets.initrd.architecture.${ARCH}.name")"
	os_version="$(get_from_kata_deps ".assets.initrd.architecture.${ARCH}.version")"
	if [[ -n "${variant}" ]]; then
		initrd_type+="-${variant}"
		os_name="$(get_from_kata_deps ".assets.initrd.architecture.${ARCH}.${variant}.name")"
		os_version="$(get_from_kata_deps ".assets.initrd.architecture.${ARCH}.${variant}.version")"
	fi

	local component="rootfs-${initrd_type}"

	local osbuilder_last_commit
	osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit
	guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local libs_last_commit
	libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version
	gperf_version="$(get_from_kata_deps ".externals.gperf.version")"
	local libseccomp_version
	libseccomp_version="$(get_from_kata_deps ".externals.libseccomp.version")"
	local rust_version
	rust_version="$(get_from_kata_deps ".languages.rust.meta.newest-version")"
	local agent_last_commit
	agent_last_commit=$(merge_two_hashes \
		"$(get_last_modification "${repo_root_dir}/src/agent")" \
		"$(get_last_modification "${repo_root_dir}/tools/packaging/static-build/agent")")

	latest_artefact="$(get_kata_version)-${os_name}-${os_version}-${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${initrd_type}"
	if [[ "${variant}" == *confidential ]]; then
		# For the confidential initrd we depend on the kernel built in order to ensure that
		# measured boot is used
		if [[ "${variant}" == "nvidia-gpu-confidential" ]]; then
			latest_artefact+="-$(get_latest_kernel_nvidia_artefact_and_builder_image_version)"
			latest_artefact+="-$(get_latest_nvidia_driver_version)"
			latest_artefact+="-$(get_latest_nvidia_ctk_version)"
			latest_artefact+="-$(get_latest_nvidia_nvrc_version)"
			latest_artefact+="-$(get_latest_nvidia_nvat_version)"
		else
			latest_artefact+="-$(get_latest_kernel_artefact_and_builder_image_version)"
		fi
		latest_artefact+="-$(get_latest_coco_guest_components_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_pause_image_artefact_and_builder_image_version)"
	fi

	if [[ "${variant}" == "nvidia-gpu" ]]; then
		# If we bump the kernel we need to rebuild the initrd as well
		latest_artefact+="-$(get_latest_kernel_nvidia_artefact_and_builder_image_version)"
		latest_artefact+="-$(get_latest_nvidia_driver_version)"
		latest_artefact+="-$(get_latest_nvidia_ctk_version)"
		latest_artefact+="-$(get_latest_nvidia_nvrc_version)"
	fi

	latest_builder_image=""

	# shellcheck disable=SC2154
	[[ "${ARCH}" == "aarch64" && "${CROSS_BUILD}" == "true" ]] && echo "warning: Don't cross build initrd for aarch64 as it's too slow" && exit 0

	install_cached_tarball_component \
		"${component}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "Create initrd"

	if [[ -n "${variant}" ]]; then
		if [[ "${variant}" == *confidential ]]; then
			COCO_GUEST_COMPONENTS_TARBALL="$(get_coco_guest_components_tarball_path)"
			export COCO_GUEST_COMPONENTS_TARBALL
			PAUSE_IMAGE_TARBALL="$(get_pause_image_tarball_path)"
			export PAUSE_IMAGE_TARBALL
		fi
	else
		# Vanilla initrd uses kata-agent as /sbin/init (no systemd).
		export AGENT_INIT=yes
	fi

	AGENT_TARBALL=$(get_agent_tarball_path)
	export AGENT_TARBALL
	export AGENT_POLICY

	if [[ -n "${GUEST_HOOKS_TARBALL_NAME}" ]]; then
		GUEST_HOOKS_TARBALL="$(get_guest_hooks_tarball_path)"
		export GUEST_HOOKS_TARBALL
	fi

	if [[ -n "${EXTRA_PKGS}" ]]; then
		export EXTRA_PKGS
	fi

	if [[ -n "${REPO_URL}" ]]; then
		export REPO_URL
	fi

	if [[ -n "${REPO_URL_X86_64}" ]]; then
		export REPO_URL_X86_64
	fi

	if [[ -n "${REPO_COMPONENTS}" ]]; then
		export REPO_COMPONENTS
	fi

	"${rootfs_builder}" --osname="${os_name}" --osversion="${os_version}" --imagetype=initrd --prefix="${prefix}" --destdir="${destdir}" --image_initrd_suffix="${variant}"
}

#Install guest initrd for confidential guests
install_initrd_confidential() {
	export CONFIDENTIAL_GUEST="yes"
	export MEASURED_ROOTFS="no"
	install_initrd "confidential"
}

# For all nvidia_gpu targets we can customize the stack that is enbled
# in the VM by setting the NVIDIA_GPU_STACK= environment variable
#
# driver       -> driver version is set via versions.yaml making sure kernel
#                 and rootfs builds are using the same version
# compute      -> enable the compute GPU stack, includes utility
# graphics     -> enable the graphics GPU stack, includes compute
# dcgm         -> enable the DCGM stack + DGCM exporter
# nvswitch     -> enable DGX like systems
# gpudirect    -> enable use-cases like GPUDirect RDMA, GPUDirect GDS
# dragonball   -> enable dragonball support
# devkit       -> builds a developer kit image, resulting in a larger
#                 rootfs size. May require incrementing the
#                 default_memory allocation and with this, potentially
#                 podOverhead. Experimental. Not for use in production
#
# The full stack can be enabled by setting all the options like:
#
# NVIDIA_GPU_STACK="compute,dcgm,nvswitch,gpudirect"
#
# Install NVIDIA GPU image
install_image_nvidia_gpu() {
	export AGENT_POLICY
	export MEASURED_ROOTFS="yes"
	export FS_TYPE="erofs"
	export SKIP_DAX_HEADER="yes"
	local version
	version=$(get_latest_nvidia_driver_version)
	EXTRA_PKGS="apt curl ${EXTRA_PKGS}"
	NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:-"driver=${version},compute,dcgm,nvswitch"}
	install_image "nvidia-gpu"
}

# Instal NVIDIA GPU confidential image
install_image_nvidia_gpu_confidential() {
	export CONFIDENTIAL_GUEST="yes"
	export AGENT_POLICY
	export MEASURED_ROOTFS="yes"
	export FS_TYPE="erofs"
	export SKIP_DAX_HEADER="yes"
	local version
	version=$(get_latest_nvidia_driver_version)
	EXTRA_PKGS="apt curl ${EXTRA_PKGS}"
	NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:-"driver=${version},compute,dcgm,nvswitch"}
	install_image "nvidia-gpu-confidential"
}

# Install the driver-agnostic nvidia base image: the NVRC-init half of the
# chiseled NVIDIA tree (see docs/design/composable-vm-images.md).
# The driver still has to be installed to build the shared stage-one (the GPU
# files are carved out afterwards), so keep the same NVIDIA_GPU_STACK as the
# monolith.
install_image_nvidia() {
	export AGENT_POLICY
	export MEASURED_ROOTFS="yes"
	export FS_TYPE="erofs"
	export SKIP_DAX_HEADER="yes"
	local version
	version=$(get_latest_nvidia_driver_version)
	EXTRA_PKGS="apt curl ${EXTRA_PKGS}"
	NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:-"driver=${version},compute,dcgm,nvswitch"}
	install_image "nvidia"
}

# Install the gpu extension image: the driver half of the chiseled NVIDIA tree,
# laid out for /run/kata-extensions/gpu (see
# docs/design/composable-vm-images.md).  It is an erofs+verity image
# (MEASURED_ROOTFS) and is driver-versioned, so multiple driver extensions can
# coexist against a single nvidia base image.
install_image_nvidia_gpu_extension() {
	# The gpu extension ships no kata-agent, so there is no agent to enforce a
	# policy: disable it (install_image defaults AGENT_POLICY to "yes").
	export AGENT_POLICY="no"
	export MEASURED_ROOTFS="yes"
	export FS_TYPE="erofs"
	export SKIP_DAX_HEADER="yes"
	# The gpu extension is GPU-userspace-only content mounted into the nvidia base image; it
	# ships no /sbin/init, so skip the rootfs init/agent sanity check.
	export SKIP_ROOTFS_CHECK="yes"
	local version
	version=$(get_latest_nvidia_driver_version)
	EXTRA_PKGS="apt curl ${EXTRA_PKGS}"
	NVIDIA_GPU_STACK=${NVIDIA_GPU_STACK:-"driver=${version},compute,dcgm,nvswitch"}
	install_image "nvidia-gpu-extension"
}

install_se_image() {
	# shellcheck disable=SC2154
	info "Create IBM SE image configured with AA_KBC=${AA_KBC}"
	"${se_image_builder}" --destdir="${destdir}"
}

#Install kernel component helper
install_cached_kernel_tarball_component() {
	local kernel_name=${1}
	local extra_tarballs="${2:-}"

	latest_artefact="${kernel_version}-${kernel_kata_config_version}-$(get_last_modification "$(dirname "${kernel_builder}")")"
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
			local modules_dir
			modules_dir=$(get_kernel_modules_dir "${kernel_version}" "${kernel_kata_config_version}" "${build_target}")

			mkdir -p "${modules_dir}" || true
			tar --strip-components=1 --zstd -xvf "${workdir}/kata-static-${kernel_name}-modules.tar.zst" -C "${modules_dir}" || return 1
			;;
		"kernel"*"-confidential")
			local modules_dir
			modules_dir=$(get_kernel_modules_dir "${kernel_version}" "${kernel_kata_config_version}" "${build_target}")
			mkdir -p "${modules_dir}" || true
			tar --zstd -xvf "${workdir}/kata-static-${kernel_name}-modules.tar.zst" -C "${modules_dir}" || return 1
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

	kernel_version="$(get_from_kata_deps ".${kernel_yaml_path}.version")"
	export kernel_version
	kernel_url="$(get_from_kata_deps ".${kernel_yaml_path}.url")"
	export kernel_url
	kernel_ref="$(get_from_kata_deps ".${kernel_yaml_path}.ref")"
	export kernel_ref
	kernel_kata_config_version="$(cat "${repo_root_dir}"/tools/packaging/kernel/kata_config_version)"
	export kernel_kata_config_version

	if [[ "${kernel_name}" == "kernel-nvidia-gpu" ]]; then
		kernel_version="$(get_from_kata_deps .assets.kernel.nvidia.version)"
		kernel_url="$(get_from_kata_deps .assets.kernel.nvidia.url)"
	fi

	case ${kernel_name} in
		kernel-nvidia-gpu|kernel-nvidia-gpu-dragonball-experimental|kernel*-confidential)
			local kernel_modules_tarball_name="kata-static-${kernel_name}-modules.tar.zst"
			local kernel_modules_tarball_path="${workdir}/${kernel_modules_tarball_name}"
			extra_tarballs="${kernel_modules_tarball_name}:${kernel_modules_tarball_path}"
			;;
	esac

	# shellcheck disable=SC2034
	default_patches_dir="${repo_root_dir}/tools/packaging/kernel/patches"

	install_cached_kernel_tarball_component "${kernel_name}" "${extra_tarballs}" && return 0

	info "build ${kernel_name}"
	info "Kernel version ${kernel_version}"
	if [[ -n "${kernel_ref}" ]]; then
		extra_cmd+=" -r ${kernel_ref}"
	fi
	DESTDIR="${destdir}" PREFIX="${prefix}" "${kernel_builder}" -v "${kernel_version}" -f -u "${kernel_url}" "${extra_cmd}"
}

#Install kernel asset (on x86_64, s390x, and aarch64 built with -x for TEE/confidential)
install_kernel() {
	local extra_cmd=""
	case "${ARCH}" in
		s390x)
			export CONFIDENTIAL_GUEST="yes"
			export MEASURED_ROOTFS="no"
			extra_cmd="-x"
			;;
		aarch64)
			export CONFIDENTIAL_GUEST="yes"
			export MEASURED_ROOTFS="yes"
			extra_cmd="-x"
			;;
		x86_64)
			export CONFIDENTIAL_GUEST="yes"
			export MEASURED_ROOTFS="yes"
			extra_cmd="-x"
			;;
	esac
	install_kernel_helper \
		"assets.kernel" \
		"kernel" \
		"${extra_cmd}"
}

install_kernel_debug() {
	export KERNEL_DEBUG_ENABLED="yes"

	install_kernel_helper \
		"assets.kernel" \
		"kernel-debug" \
		""
}

install_kernel_cca_confidential() {
	export CONFIDENTIAL_GUEST="yes"
	export MEASURED_ROOTFS="yes"

	install_kernel_helper \
		"assets.kernel-arm-experimental.confidential" \
		"kernel-confidential" \
		"-x -H deb"
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
		"-e -t dragonball -g nvidia"
}

#Install GPU enabled kernel asset
install_kernel_nvidia_gpu() {
	export CONFIDENTIAL_GUEST="yes"
	export MEASURED_ROOTFS="yes"
	install_kernel_helper \
		"assets.kernel.nvidia" \
		"kernel-nvidia-gpu" \
		"-x -g nvidia"
}

install_qemu_helper() {
	local qemu_repo_yaml_path="${1}"
	local qemu_version_yaml_path="${2}"
	local qemu_name="${3}"
	local builder="${4}"
	local qemu_tarball_name="${qemu_tarball_name:-kata-static-qemu.tar.gz}"

	qemu_repo="$(get_from_kata_deps ".${qemu_repo_yaml_path}")"
	export qemu_repo
	qemu_version="$(get_from_kata_deps ".${qemu_version_yaml_path}")"
	export qemu_version

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
	tar --zstd -xvf "${qemu_tarball_name}" -C "${destdir}"
}

# Install static qemu asset
install_qemu() {
	install_qemu_helper \
		"assets.hypervisor.qemu.url" \
		"assets.hypervisor.qemu.version" \
		"qemu" \
		"${qemu_builder}"
}

install_qemu_cca_experimental() {
	export qemu_suffix="cca-experimental"
	export qemu_tarball_name="kata-static-qemu-${qemu_suffix}.tar.gz"

	install_qemu_helper \
		"assets.hypervisor.qemu-${qemu_suffix}.url" \
		"assets.hypervisor.qemu-${qemu_suffix}.tag" \
		"qemu-${qemu_suffix}" \
		"${qemu_experimental_builder}"
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

install_qemu_tdx_experimental() {
	export qemu_suffix="tdx-experimental"
	export qemu_tarball_name="kata-static-qemu-${qemu_suffix}.tar.gz"

	install_qemu_helper \
		"assets.hypervisor.qemu-${qemu_suffix}.url" \
		"assets.hypervisor.qemu-${qemu_suffix}.tag" \
		"qemu-${qemu_suffix}" \
		"${qemu_experimental_builder}"
}

# Install static firecracker asset
install_firecracker() {
	local firecracker_version
	firecracker_version=$(get_from_kata_deps ".assets.hypervisor.firecracker.version")

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
	install -D --mode "${default_binary_permissions}" "release-${firecracker_version}-${ARCH}/firecracker-${firecracker_version}-${ARCH}" "${destdir}/opt/kata/bin/firecracker"
	install -D --mode "${default_binary_permissions}" "release-${firecracker_version}-${ARCH}/jailer-${firecracker_version}-${ARCH}" "${destdir}/opt/kata/bin/jailer"
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
	install -D --mode "${default_binary_permissions}" cloud-hypervisor/cloud-hypervisor "${destdir}/opt/kata/bin/cloud-hypervisor${suffix}"
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

# Install static stratovirt asset
install_stratovirt() {
	local stratovirt_version
	stratovirt_version=$(get_from_kata_deps ".assets.hypervisor.stratovirt.version")

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
	install -D --mode "${default_binary_permissions}" static-stratovirt/stratovirt "${destdir}/opt/kata/bin/stratovirt"
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
	install -D --mode "${default_binary_permissions}" virtiofsd/virtiofsd "${destdir}/opt/kata/libexec/virtiofsd"
}

# Install static nydus asset
install_nydus() {
	[[ "${ARCH}" == "aarch64" ]] && ARCH=arm64

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
	install -D --mode "${default_binary_permissions}" nydus-static/nydusd "${destdir}/opt/kata/libexec/nydusd"
}

# Shared helper: extract measured-rootfs root hashes from confidential image tarballs.
# These are needed by the Rust runtime (runtime-rs) at build time for dm-verity.
_collect_root_hashes() {
	for variant in "${MEASURED_ROOTFS_VARIANTS[@]}"; do
		# The measured base image ships as kata-static-rootfs-image.tar.zst
		# (no variant suffix), but carries its root hash under the "base" label.
		local tarball_glob="kata-static-rootfs-image-${variant}.tar.zst"
		[[ "${variant}" == "base" ]] && tarball_glob="kata-static-rootfs-image.tar.zst"
		local image_conf_tarball
		image_conf_tarball="$(find "${workdir}" -maxdepth 1 -name "${tarball_glob}" 2>/dev/null | head -n 1)"
		# Only one variant may be built at a time so we need to
		# skip one or the other if not available.
		[[ -f "${image_conf_tarball}" ]] || continue

		local root_hash_basedir="./opt/kata/share/kata-containers/"
		local root_hash_path="${root_hash_basedir}root_hash_${variant}.txt"
		tar --zstd -tf "${image_conf_tarball}" "${root_hash_path}" >/dev/null 2>&1 || continue
		if ! tar --zstd -xvf "${image_conf_tarball}" --transform s,"${root_hash_basedir}",, "${root_hash_path}"; then
			die "Cannot extract root hash from ${image_conf_tarball}"
		fi

		mv "root_hash_${variant}.txt" "${workdir}/root_hash_${variant}.txt"
	done
}

# Install the Go shim only (containerd-shim-kata-v2 Go runtime + kata-runtime + Go configs).
install_shim_v2_go() {
	local shim_v2_last_commit
	shim_v2_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime")"
	local protocols_last_commit
	protocols_last_commit="$(get_last_modification "${repo_root_dir}/src/libs/protocols")"
	local GO_VERSION
	GO_VERSION="$(get_from_kata_deps ".languages.golang.meta.newest-version")"
	local RUST_VERSION
	RUST_VERSION="$(get_from_kata_deps ".languages.rust.meta.newest-version")"

	latest_artefact="$(get_kata_version)-${shim_v2_last_commit}-${protocols_last_commit}-${GO_VERSION}"
	latest_builder_image="$(get_shim_v2_image_name)"

	install_cached_tarball_component \
		"shim-v2-go" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	export GO_VERSION
	export RUST_VERSION
	export MEASURED_ROOTFS
	RUNTIME_CHOICE="go"
	export RUNTIME_CHOICE

	_collect_root_hashes

	DESTDIR="${destdir}" PREFIX="${prefix}" "${shimv2_builder}"
}

# Install the Rust shim only (containerd-shim-kata-v2 runtime-rs + runtime-rs configs).
install_shim_v2_rust() {
	local runtime_rs_last_commit
	runtime_rs_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime-rs")"
	local protocols_last_commit
	protocols_last_commit="$(get_last_modification "${repo_root_dir}/src/libs/protocols")"
	local GO_VERSION
	GO_VERSION="$(get_from_kata_deps ".languages.golang.meta.newest-version")"
	local RUST_VERSION
	RUST_VERSION="$(get_from_kata_deps ".languages.rust.meta.newest-version")"

	latest_artefact="$(get_kata_version)-${runtime_rs_last_commit}-${protocols_last_commit}-${RUST_VERSION}"
	latest_builder_image="$(get_shim_v2_image_name)"

	install_cached_tarball_component \
		"shim-v2-rust" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	export GO_VERSION
	export RUST_VERSION
	export MEASURED_ROOTFS
	RUNTIME_CHOICE="rust"
	export RUNTIME_CHOICE

	_collect_root_hashes

	DESTDIR="${destdir}" PREFIX="${prefix}" "${shimv2_builder}"
}

install_ovmf() {
	ovmf_type="${1:-x86_64}"
	tarball_name="${2:-edk2-x86_64.tar.gz}"
	if [[ "${ARCH}" == "aarch64" ]]; then
	  if [[ "${ovmf_type}" != "cca" ]]; then
		  ovmf_type="arm64"
		  tarball_name="edk2-arm64.tar.gz"
		fi
	fi

	local component_name="ovmf"
	[[ "${ovmf_type}" == "sev" ]] && component_name="ovmf-sev"
	[[ "${ovmf_type}" == "tdx" ]] && component_name="ovmf-tdx"

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
	tar --zstd -xvf "${builddir}/${tarball_name}" -C "${destdir}"
}

# Install OVMF SEV
install_ovmf_sev() {
	install_ovmf "sev" "edk2-sev.tar.gz"
}

# Install OVMF TDX
install_ovmf_tdx() {
	install_ovmf "tdx" "edk2-tdx.tar.gz"
}

# Install OVMF CCA
install_ovmf_cca() {
	install_ovmf "cca" "edk2-cca.tar.gz"
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
	latest_artefact="$(get_kata_version)-$(git log -1 --abbrev=9 --pretty=format:"%h" "${repo_root_dir}"/src/agent)"
	latest_builder_image="$(get_agent_image_name)"

	install_cached_tarball_component \
		"${build_target}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	LIBSECCOMP_VERSION="$(get_from_kata_deps ".externals.libseccomp.version")"
	export LIBSECCOMP_VERSION
	LIBSECCOMP_URL="$(get_from_kata_deps ".externals.libseccomp.url")"
	export LIBSECCOMP_URL
	GPERF_VERSION="$(get_from_kata_deps ".externals.gperf.version")"
	export GPERF_VERSION
	GPERF_URL="$(get_from_kata_deps ".externals.gperf.url")"
	export GPERF_URL

	info "build static agent"
	DESTDIR="${destdir}" AGENT_POLICY="${AGENT_POLICY}" "${agent_builder}"
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
	[[ -n "${script}" ]] || die "need script"

	local script_path

	# If the script isn't specified as an absolute or relative path,
	# find it.
	if grep -q '/' <<< "${script}"
	then
		script_path="${script}"
	else
		script_path=$(find "${repo_root_dir}/" -type f -name "${script}")
	fi

	local script_file
	script_file=$(basename "${script_path}")

	local script_file_name

	# Remove any extension
	script_file_name="${script_file%%.*}"

	info "installing utility script ${script}"

	local bin_dir
	bin_dir="${destdir}/opt/kata/bin/"

	mkdir -p "${bin_dir}"

	install -D \
		--mode "${default_binary_permissions}" \
		"${script_path}" \
		"${bin_dir}/${script_file}"

	[[ "${script_file}" = "${script_file_name}" ]] && return 0

	pushd "${bin_dir}" &>/dev/null

	# Create a sym-link with the extension removed
	ln -sf "${script_file}" "${script_file_name}"

	popd &>/dev/null
}

install_tools_helper() {
	tool=${1}

	latest_artefact="$(get_kata_version)-$(git log -1 --abbrev=9 --pretty=format:"%h" "${repo_root_dir}"/src/tools/"${tool}")"
	latest_builder_image="$(get_tools_image_name)"

	install_cached_tarball_component \
		"${tool}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	LIBSECCOMP_VERSION="$(get_from_kata_deps ".externals.libseccomp.version")"
	export LIBSECCOMP_VERSION
	LIBSECCOMP_URL="$(get_from_kata_deps ".externals.libseccomp.url")"
	export LIBSECCOMP_URL
	GPERF_VERSION="$(get_from_kata_deps ".externals.gperf.version")"
	export GPERF_VERSION
	GPERF_URL="$(get_from_kata_deps ".externals.gperf.url")"
	export GPERF_URL

	info "build static ${tool}"
	"${tools_builder}" "${tool}"

	tool_binary=${tool}
	[[ "${tool}" = "agent-ctl" ]] && tool_binary="kata-agent-ctl"
	[[ "${tool}" = "trace-forwarder" ]] && tool_binary="kata-trace-forwarder"

	local tool_build_dir="target"
	binary=$(find "${repo_root_dir}/${tool_build_dir}" -type f -name "${tool_binary}")

	binary_count=$(echo "${binary}" | grep -c '^' || echo "0")
	if [[ "${binary}" = "" ]]; then
		die "No binary found for ${tool} in ${repo_root_dir}/${tool_build_dir} (expected: ${tool_binary})."
	elif [[ "${binary_count}" -gt 1 ]]; then
		die "Multiple binaries found for ${tool} (expected single ${tool_binary}). Found:"$'\n'"${binary}"
	fi

	if [[ "${tool}" == "genpolicy" ]]; then
		defaults_path="${destdir}/opt/kata/share/defaults/kata-containers"
		mkdir -p "${defaults_path}"
		install -D --mode 0644 "${repo_root_dir}/src/tools/${tool}/rules.rego" "${defaults_path}/rules.rego"
		install -D --mode 0644 "${repo_root_dir}/src/tools/${tool}/genpolicy-settings.json" "${defaults_path}/genpolicy-settings.json"
		mkdir -p "${defaults_path}/genpolicy-settings.d"
		# Scenario drop-in examples (10-*.json base, 20-*.json overlays). Do not ship test drop-ins (99-*).
		drop_in_examples="${repo_root_dir}/src/tools/${tool}/drop-in-examples"
		if [[ -d "${drop_in_examples}" ]]; then
			mkdir -p "${defaults_path}/drop-in-examples"
			for f in "${drop_in_examples}"/10-*.json "${drop_in_examples}"/20-*.json; do
				[[ -e "${f}" ]] && install -D --mode 0644 "${f}" "${defaults_path}/drop-in-examples/$(basename "${f}")"
			done
			[[ -f "${drop_in_examples}/README.md" ]] && install -D --mode 0644 "${drop_in_examples}/README.md" "${defaults_path}/drop-in-examples/README.md"
		fi
		binary_permissions="0755"
	else
		binary_permissions="${default_binary_permissions}"
	fi

	if [[ "${tool}" == "agent-ctl" ]]; then
		defaults_path="${destdir}/opt/kata/share/defaults/kata-containers/agent-ctl"
		mkdir -p "${defaults_path}"
		install -D --mode 0644 "${repo_root_dir}/src/tools/${tool}/template/oci_config.json" "${defaults_path}/oci_config.json"
	fi

	info "Install static ${tool_binary}"
	mkdir -p "${destdir}/opt/kata/bin/"
	install -D --mode "${binary_permissions}" "${binary}" "${destdir}/opt/kata/bin/${tool_binary}"
}

install_agent_ctl() {
	install_tools_helper "agent-ctl"
}

install_genpolicy() {
	install_tools_helper "genpolicy"
}

install_kata_ctl() {
	install_tools_helper "kata-ctl"
}

install_kata_manager() {
	install_script_helper "kata-manager.sh"
}

install_trace_forwarder() {
	install_tools_helper "trace-forwarder"
}

get_kata_version() {
	local v
	v=$(cat "${version_file}")
	echo "${v}"
}

handle_build() {
	info "DESTDIR ${destdir}"

	latest_artefact=""
	latest_builder_image=""

	local build_target
	build_target="$1"

	export final_tarball_path="${workdir}/kata-static-${build_target}.tar.zst"
	final_tarball_name="$(basename "${final_tarball_path}")"
	export final_tarball_name
	rm -f "${final_tarball_name}"

	case "${build_target}" in
	all)
		install_agent_ctl
		install_clh
		install_firecracker
		install_image
		install_image_confidential
		install_image_mariner
		install_initrd
		install_initrd_confidential
		install_kata_ctl
		install_kata_manager
		install_kernel
		install_kernel_cca_confidential
		install_kernel_dragonball_experimental
		install_log_parser_rs
		install_nydus
		install_ovmf
		install_ovmf_sev
		install_ovmf_tdx
		install_qemu
		install_qemu_snp_experimental
		install_qemu_tdx_experimental
		install_stratovirt
		install_shim_v2_go
		install_shim_v2_rust
		install_trace_forwarder
		install_virtiofsd
		;;

	agent) install_agent ;;

	agent-ctl) install_agent_ctl ;;

	busybox) install_busybox ;;

	boot-image-se) install_se_image ;;

	coco-guest-components) install_coco_guest_components ;;

	cloud-hypervisor) install_clh ;;

	firecracker) install_firecracker ;;

	genpolicy) install_genpolicy ;;

	kata-ctl) install_kata_ctl ;;

	kata-manager) install_kata_manager ;;

	kernel) install_kernel ;;

	kernel-debug) install_kernel_debug ;;

	kernel-cca-confidential) install_kernel_cca_confidential ;;

	kernel-dragonball-experimental) install_kernel_dragonball_experimental ;;

	kernel-nvidia-gpu-dragonball-experimental) install_kernel_nvidia_gpu_dragonball_experimental ;;

	kernel-nvidia-gpu) install_kernel_nvidia_gpu ;;

	nydus) install_nydus ;;

	ovmf) install_ovmf ;;

	ovmf-sev) install_ovmf_sev ;;

	ovmf-tdx) install_ovmf_tdx ;;

	ovmf-cca) install_ovmf_cca ;;

	pause-image) install_pause_image ;;

	qemu) install_qemu ;;

	qemu-cca-experimental) install_qemu_cca_experimental ;;

	qemu-snp-experimental) install_qemu_snp_experimental ;;

	qemu-tdx-experimental) install_qemu_tdx_experimental ;;

	stratovirt) install_stratovirt ;;

	rootfs-image) install_image_base ;;

	rootfs-image-confidential) install_image_confidential ;;

	rootfs-image-coco-extension) install_image_coco_extension ;;

	rootfs-image-mariner) install_image_mariner ;;

	rootfs-initrd) install_initrd ;;

	rootfs-initrd-confidential) install_initrd_confidential ;;

	rootfs-image-nvidia-gpu) install_image_nvidia_gpu ;;

	rootfs-image-nvidia-gpu-confidential) install_image_nvidia_gpu_confidential ;;

	rootfs-image-nvidia) install_image_nvidia ;;

	rootfs-image-nvidia-gpu-extension) install_image_nvidia_gpu_extension ;;

	rootfs-cca-confidential-image) install_image_confidential ;;

	rootfs-cca-confidential-initrd) install_initrd_confidential ;;

	shim-v2-go) install_shim_v2_go ;;

	shim-v2-rust) install_shim_v2_rust ;;

	trace-forwarder) install_trace_forwarder ;;

	virtiofsd) install_virtiofsd ;;

	dummy)
		tar --zstd -cvf "${final_tarball_path}" --files-from /dev/null
	       	;;

	*)
		die "Invalid build target ${build_target}"
		;;
	esac

	if [[ ! -f "${final_tarball_path}" ]]; then
		cd "${destdir}"
		tar --zstd -cvf "${final_tarball_path}" "."
	fi
	tar --zstd -tvf "${final_tarball_path}"

	case ${build_target} in
		kernel-nvidia-gpu|kernel-nvidia-gpu-dragonball-experimental)
			local modules_final_tarball_path="${workdir}/kata-static-${build_target}-modules.tar.zst"
			if [[ ! -f "${modules_final_tarball_path}" ]]; then
				local modules_dir
				modules_dir=$(get_kernel_modules_dir "${kernel_version}" "${kernel_kata_config_version}" "${build_target}")

				parent_dir=$(dirname "${modules_dir}")
				parent_dir_basename=$(basename "${parent_dir}")

				pushd "${parent_dir}"
				rm -f "${parent_dir_basename}"/build
				tar --zstd -cvf "${modules_final_tarball_path}" "."
				popd
			fi
			tar --zstd -tvf "${modules_final_tarball_path}"
			;;
		kernel*-confidential)
			local modules_final_tarball_path="${workdir}/kata-static-${build_target}-modules.tar.zst"
			if [[ ! -f "${modules_final_tarball_path}" ]]; then
				local modules_dir
				modules_dir=$(get_kernel_modules_dir "${kernel_version}" "${kernel_kata_config_version}" "${build_target}")

				pushd "${modules_dir}"
				rm -f build
				tar --zstd -cvf "${modules_final_tarball_path}" "."
				popd
			fi
			tar --zstd -tvf "${modules_final_tarball_path}"
			;;
		shim-v2-go|shim-v2-rust)
			if [[ "${MEASURED_ROOTFS}" == "yes" ]]; then
				for variant in "${MEASURED_ROOTFS_VARIANTS[@]}"; do
					[[ -f "${workdir}/root_hash_${variant}.txt" ]] && mv "${workdir}/root_hash_${variant}.txt" "${workdir}/${build_target}-root_hash_${variant}.txt"
				done
			fi
			;;
	esac

	pushd "${workdir}"
	echo "${latest_artefact}-$(git log -1 --abbrev=9 --pretty=format:"%h" "${repo_root_dir}"/tools/packaging/kata-deploy/local-build)" > "${build_target}"-version
	echo "${latest_builder_image}" > "${build_target}"-builder-image-version
	sha256sum "${final_tarball_name}" > "${build_target}"-sha256sum

	if [[ "${PUSH_TO_REGISTRY}" = "yes" ]]; then
		if [[ -z "${ARTEFACT_REGISTRY}" ]] ||
			[[ -z "${ARTEFACT_REPOSITORY}" ]] ||
			[[ -z "${ARTEFACT_REGISTRY_USERNAME}" ]] ||
			[[ -z "${ARTEFACT_REGISTRY_PASSWORD}" ]] ||
		      	[[ -z "${TARGET_BRANCH}" ]]; then
			die "ARTEFACT_REGISTRY, ARTEFACT_REPOSITORY, ARTEFACT_REGISTRY_USERNAME, ARTEFACT_REGISTRY_PASSWORD and TARGET_BRANCH must be passed to the script when pushing the artefacts to the registry!"
		fi

		echo "${ARTEFACT_REGISTRY_PASSWORD}" | oras login "${ARTEFACT_REGISTRY}" -u "${ARTEFACT_REGISTRY_USERNAME}" --password-stdin

		tags=(latest-"${TARGET_BRANCH}")

		# Always tag with HEAD commit SHA to ensure all components are traceable
		# to the exact repository state, regardless of which files were modified
		head_sha="$(git -C "${repo_root_dir}" log -1 --pretty=format:"%H")"
		tags+=("${head_sha}")

		# Add component-specific tag if set and different from HEAD SHA
		if [[ -n "${artefact_tag:-}" && "${artefact_tag}" != "${head_sha}" ]]; then
			tags+=("${artefact_tag}")
		fi
		if [[ "${RELEASE}" == "yes" ]]; then
			tags+=("$(cat "${version_file}")")
		fi

		echo "Pushing ${build_target} with tags: ${tags[*]}"

		normalized_tags=""
		for tag in "${tags[@]}"; do
			# tags can only contain lowercase and uppercase letters, digits, underscores, periods, and hyphens
			# and are limited to 128 characters. Sanitize via the shared helper
			# (the pull path uses the same helper) and trim down to leave room
			# for the arch suffix.
			tag_length_limit="$((128 - $(echo "-$(uname -m)" | wc -c)))"
			normalized_tag="$(sanitize_tag_component "${tag}" | head -c "${tag_length_limit}")-$(uname -m)"
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
			kernel-nvidia-gpu|kernel-nvidia-gpu-dragonball-experimental|kernel*-confidential)
				files_to_push+=(
					"kata-static-${build_target}-modules.tar.zst"
				)
				;;
		shim-v2-go|shim-v2-rust)
			if [[ "${MEASURED_ROOTFS}" == "yes" ]]; then
				local found_any=""
				for variant in "${MEASURED_ROOTFS_VARIANTS[@]}"; do
					# The variants could be built independently we need to check if
					# they exist and then push them to the registry
					[[ -f "${workdir}/${build_target}-root_hash_${variant}.txt" ]] && files_to_push+=("${build_target}-root_hash_${variant}.txt") && found_any="yes"
				done
				[[ -z "${found_any}" ]] && die "No files to push for ${build_target} with MEASURED_ROOTFS support"
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
	exec 1>&"${stdout}"
	exec 2>&"${stderr}"
	error "Failed to build: ${t}, logs:"
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
		shim-v2-go
		shim-v2-rust
		trace-forwarder
		virtiofsd
		dummy
	)
	silent=false
	while getopts "hs-:" opt; do
		case ${opt} in
		-)
			case "${OPTARG}" in
			build=*)
				# shellcheck disable=SC2206
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
		if [[ "${silent}" == true ]]; then
			log_file="${builddir}/log"
			echo "build log: ${log_file}"
		fi
		(
			cd "${builddir}"
			if [[ "${silent}" == true ]]; then
				local stdout
				local stderr
				# Save stdout and stderr, to be restored
				# by silent_mode_error_trap() in case of
				# build failure.
				exec {stdout}>&1
				exec {stderr}>&2
				# shellcheck disable=SC2064
				trap "silent_mode_error_trap ${stdout} ${stderr} ${t} \"${log_file}\"" ERR
				handle_build "${t}" &>"${log_file}"
			else
				handle_build "${t}"
			fi
		)
	done

}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
	main "$@"
fi
