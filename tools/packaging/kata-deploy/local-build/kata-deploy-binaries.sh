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

readonly agent_builder="${static_build_dir}/agent/build.sh"
readonly clh_builder="${static_build_dir}/cloud-hypervisor/build-static-clh.sh"
readonly firecracker_builder="${static_build_dir}/firecracker/build-static-firecracker.sh"
readonly kernel_builder="${static_build_dir}/kernel/build.sh"
readonly ovmf_builder="${static_build_dir}/ovmf/build.sh"
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
MEASURED_ROOTFS=${MEASURED_ROOTFS:-no}
USE_CACHE="${USE_CACHE:-"yes"}"
ARTEFACT_REGISTRY="${ARTEFACT_REGISTRY:-ghcr.io}"
ARTEFACT_REGISTRY_USERNAME="${ARTEFACT_REGISTRY_USERNAME:-}"
ARTEFACT_REGISTRY_PASSWORD="${ARTEFACT_REGISTRY_PASSWORD:-}"
TARGET_BRANCH="${TARGET_BRANCH:-main}"
PUSH_TO_REGISTRY="${PUSH_TO_REGISTRY:-}"

workdir="${WORKDIR:-$PWD}"

destdir="${workdir}/kata-static"

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
	agent-opa
	agent-ctl
	boot-image-se
	cloud-hypervisor
	cloud-hypervisor-glibc
	firecracker
	kata-ctl
	kernel
	kernel-dragonball-experimental
	kernel-experimental
	kernel-nvidia-gpu
	kernel-nvidia-gpu-snp
	kernel-nvidia-gpu-tdx-experimental
	kernel-sev-tarball
	kernel-tdx-experimental
	nydus
	ovmf
	ovmf-sev
	qemu
	qemu-snp-experimental
	qemu-tdx-experimental
	stratovirt
	rootfs-image
	rootfs-image-tdx
	rootfs-initrd
	rootfs-initrd-mariner
	rootfs-initrd-sev
	runk
	shim-v2
	tdvf
	trace-forwarder
	virtiofsd
EOF

	exit "${return_code}"
}

cleanup_and_fail() {
       rm -f "${component_tarball_name}"
       return 1
}

install_cached_tarball_component() {
	if [ "${USE_CACHE}" != "yes" ]; then
		return 1
	fi

	local component="${1}"
	local current_version="${2}"
	local current_image_version="${3}"
	local component_tarball_name="${4}"
	local component_tarball_path="${5}"

	sudo oras pull ${ARTEFACT_REGISTRY}/kata-containers/cached-artefacts/${build_target}:latest-${TARGET_BRANCH}-$(uname -m)

	cached_version="$(cat ${component}-version)"
	cached_image_version="$(cat ${component}-builder-image-version)"

	rm -f ${component}-version
	rm -f ${component}-builder-image-version

	[ "${cached_image_version}" != "${current_image_version}" ] && return 1
	[ "${cached_version}" != "${current_version}" ] && return 1
	sha256sum -c "${component}-sha256sum" || return $(cleanup_and_fail)

	info "Using cached tarball of ${component}"
	mv "${component_tarball_name}" "${component_tarball_path}"
}

#Install guest image
install_image() {
	local variant="${1:-}"

	image_type="image"
	if [ -n "${variant}" ]; then
		image_type+="-${variant}"
	fi

	local component="rootfs-${image_type}"

	local osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local agent_last_commit="$(get_last_modification "${repo_root_dir}/src/agent")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version="$(get_from_kata_deps "externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps "externals.libseccomp.version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"

	latest_artefact="${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${image_type}"
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
		os_name="$(get_from_kata_deps "assets.image.architecture.${ARCH}.${variant}.name")"
		os_version="$(get_from_kata_deps "assets.image.architecture.${ARCH}.${variant}.version")"
	else
		os_name="$(get_from_kata_deps "assets.image.architecture.${ARCH}.name")"
		os_version="$(get_from_kata_deps "assets.image.architecture.${ARCH}.version")"
	fi
	
	"${rootfs_builder}" --osname="${os_name}" --osversion="${os_version}" --imagetype=image --prefix="${prefix}" --destdir="${destdir}" --image_initrd_suffix="${variant}"
}

#Install guest image for tdx
install_image_tdx() {
	export AGENT_POLICY=yes
	install_image "tdx"
}

#Install guest initrd
install_initrd() {
	local variant="${1:-}"

	initrd_type="initrd"
	if [ -n "${variant}" ]; then
		initrd_type+="-${variant}"
	fi

	local component="rootfs-${initrd_type}"

	local osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local agent_last_commit="$(get_last_modification "${repo_root_dir}/src/agent")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version="$(get_from_kata_deps "externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps "externals.libseccomp.version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"

	latest_artefact="${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${initrd_type}"
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
		os_name="$(get_from_kata_deps "assets.initrd.architecture.${ARCH}.${variant}.name")"
		os_version="$(get_from_kata_deps "assets.initrd.architecture.${ARCH}.${variant}.version")"
	else
		os_name="$(get_from_kata_deps "assets.initrd.architecture.${ARCH}.name")"
		os_version="$(get_from_kata_deps "assets.initrd.architecture.${ARCH}.version")"
	fi

	"${rootfs_builder}" --osname="${os_name}" --osversion="${os_version}" --imagetype=initrd --prefix="${prefix}" --destdir="${destdir}" --image_initrd_suffix="${variant}"
}

#Install Mariner guest initrd
install_initrd_mariner() {
	export AGENT_POLICY=yes
	install_initrd "mariner"
}

#Install guest initrd for sev
install_initrd_sev() {
	export AGENT_POLICY=yes
	install_initrd "sev"
}

install_se_image() {
	info "Create IBM SE image configured with AA_KBC=${AA_KBC}"
	"${se_image_builder}" --destdir="${destdir}"
}

#Install kernel component helper
install_cached_kernel_tarball_component() {
	local kernel_name=${1}
	local module_dir=${2:-""}

	latest_artefact="${kernel_version}-${kernel_kata_config_version}-$(get_last_modification $(dirname $kernel_builder))"
	latest_builder_image="$(get_kernel_image_name)"

	install_cached_tarball_component \
		"${kernel_name}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		|| return 1
	
	if [[ "${kernel_name}" != "kernel-sev" ]]; then
		return 0
	fi

	# SEV specific code path
	install_cached_tarball_component \
		"${kernel_name}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"kata-static-kernel-sev-modules.tar.xz" \
		"${workdir}/kata-static-kernel-sev-modules.tar.xz" \
		|| return 1

	if [[ -n "${module_dir}" ]]; then
		mkdir -p "${module_dir}"
		tar xvf "${workdir}/kata-static-kernel-sev-modules.tar.xz" -C  "${module_dir}" && return 0
	fi

	return 1
}

#Install kernel asset
install_kernel_helper() {
	local kernel_version_yaml_path="${1}"
	local kernel_name="${2}"
	local extra_cmd="${3:-}"

	export kernel_version="$(get_from_kata_deps ${kernel_version_yaml_path})"
	export kernel_kata_config_version="$(cat ${repo_root_dir}/tools/packaging/kernel/kata_config_version)"
	local module_dir=""

	if [[ "${kernel_name}" == "kernel-sev" ]]; then
		kernel_version="$(get_from_kata_deps assets.kernel.sev.version)"
		default_patches_dir="${repo_root_dir}/tools/packaging/kernel/patches"
		module_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/kernel-sev/builddir/kata-linux-${kernel_version#v}-${kernel_kata_config_version}/lib/modules/${kernel_version#v}"
	fi

	install_cached_kernel_tarball_component ${kernel_name} ${module_dir} && return 0

	info "build ${kernel_name}"
	info "Kernel version ${kernel_version}"
	DESTDIR="${destdir}" PREFIX="${prefix}" "${kernel_builder}" -v "${kernel_version}" ${extra_cmd}
}

#Install kernel asset
install_kernel() {
	install_kernel_helper \
		"assets.kernel.version" \
		"kernel" \
		"-f"
}

install_kernel_dragonball_experimental() {
	install_kernel_helper \
		"assets.kernel-dragonball-experimental.version" \
		"kernel-dragonball-experimental" \
		"-e -t dragonball"
}

#Install GPU enabled kernel asset
install_kernel_nvidia_gpu() {
	local kernel_url="$(get_from_kata_deps assets.kernel.url)"

	install_kernel_helper \
		"assets.kernel.version" \
		"kernel-nvidia-gpu" \
		"-g nvidia -u ${kernel_url} -H deb"
}

#Install GPU and SNP enabled kernel asset
install_kernel_nvidia_gpu_snp() {
	local kernel_url="$(get_from_kata_deps assets.kernel.sev.url)"

	install_kernel_helper \
		"assets.kernel.sev.version" \
		"kernel-nvidia-gpu-snp" \
		"-x sev -g nvidia -u ${kernel_url} -H deb"
}

#Install GPU and TDX experimental enabled kernel asset
install_kernel_nvidia_gpu_tdx_experimental() {
	local kernel_url="$(get_from_kata_deps assets.kernel-tdx-experimental.url)"

	install_kernel_helper \
		"assets.kernel-tdx-experimental.version" \
		"kernel-nvidia-gpu-tdx-experimental" \
		"-x tdx -g nvidia -u ${kernel_url} -H deb"
}

#Install experimental TDX kernel asset
install_kernel_tdx_experimental() {
	local kernel_url="$(get_from_kata_deps assets.kernel-tdx-experimental.url)"

	export MEASURED_ROOTFS=yes

	install_kernel_helper \
		"assets.kernel-tdx-experimental.version" \
		"kernel-tdx-experimental" \
		"-x tdx -u ${kernel_url}"
}

#Install sev kernel asset
install_kernel_sev() {
	info "build sev kernel"
	local kernel_url="$(get_from_kata_deps assets.kernel.sev.url)"

	install_kernel_helper \
		"assets.kernel.sev.version" \
		"kernel-sev" \
		"-x sev -u ${kernel_url}"
}

install_qemu_helper() {
	local qemu_repo_yaml_path="${1}"
	local qemu_version_yaml_path="${2}"
	local qemu_name="${3}"
	local builder="${4}"
	local qemu_tarball_name="${qemu_tarball_name:-kata-static-qemu.tar.gz}"

	export qemu_repo="$(get_from_kata_deps ${qemu_repo_yaml_path})"
	export qemu_version="$(get_from_kata_deps ${qemu_version_yaml_path})"

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

install_qemu_tdx_experimental() {
	export qemu_suffix="tdx-experimental"
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

# Install static firecracker asset
install_firecracker() {
	local firecracker_version=$(get_from_kata_deps "assets.hypervisor.firecracker.version")

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
	sudo install -D --owner root --group root --mode 0744 release-${firecracker_version}-${ARCH}/firecracker-${firecracker_version}-${ARCH} "${destdir}/opt/kata/bin/firecracker"
	sudo install -D --owner root --group root --mode 0744 release-${firecracker_version}-${ARCH}/jailer-${firecracker_version}-${ARCH} "${destdir}/opt/kata/bin/jailer"
}

install_clh_helper() {
	libc="${1}"
	features="${2}"
	suffix="${3:-""}"

	latest_artefact="$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version")"
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
	sudo install -D --owner root --group root --mode 0744 cloud-hypervisor/cloud-hypervisor "${destdir}/opt/kata/bin/cloud-hypervisor${suffix}"
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
	local stratovirt_version=$(get_from_kata_deps "assets.hypervisor.stratovirt.version")

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
	sudo install -D --owner root --group root --mode 0744 static-stratovirt/stratovirt "${destdir}/opt/kata/bin/stratovirt"
}

# Install static virtiofsd asset
install_virtiofsd() {
	latest_artefact="$(get_from_kata_deps "externals.virtiofsd.version")-$(get_from_kata_deps "externals.virtiofsd.toolchain")"
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
	sudo install -D --owner root --group root --mode 0744 virtiofsd/virtiofsd "${destdir}/opt/kata/libexec/virtiofsd"
}

# Install static nydus asset
install_nydus() {
	[ "${ARCH}" == "aarch64" ] && ARCH=arm64

	latest_artefact="$(get_from_kata_deps "externals.nydus.version")"
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
	sudo install -D --owner root --group root --mode 0744 nydus-static/nydusd "${destdir}/opt/kata/libexec/nydusd"
}

#Install all components that are not assets
install_shimv2() {
	local shim_v2_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime")"
	local runtime_rs_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime-rs")"
	local protocols_last_commit="$(get_last_modification "${repo_root_dir}/src/libs/protocols")"
	local GO_VERSION="$(get_from_kata_deps "languages.golang.meta.newest-version")"
	local RUST_VERSION="$(get_from_kata_deps "languages.rust.meta.newest-version")"
	
	latest_artefact="${shim_v2_last_commit}-${protocols_last_commit}-${runtime_rs_last_commit}-${GO_VERSION}-${RUST_VERSION}"
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

	DESTDIR="${destdir}" PREFIX="${prefix}" "${shimv2_builder}"
}

install_ovmf() {
	ovmf_type="${1:-x86_64}"
	tarball_name="${2:-edk2-x86_64.tar.gz}"

	local component_name="ovmf"
	[ "${ovmf_type}" == "sev" ] && component_name="ovmf-sev"
	[ "${ovmf_type}" == "tdx" ] && component_name="tdvf"

	latest_artefact="$(get_from_kata_deps "externals.ovmf.${ovmf_type}.version")"
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

# Install TDVF
install_tdvf() {
	install_ovmf "tdx" "edk2-tdx.tar.gz"
}

# Install OVMF SEV
install_ovmf_sev() {
	install_ovmf "sev" "edk2-sev.tar.gz"
}

install_agent_helper() {
	agent_policy="${1:-no}"

	latest_artefact="$(git log -1 --pretty=format:"%h" ${repo_root_dir}/src/agent)"
	latest_builder_image="$(get_agent_image_name)"

	install_cached_tarball_component \
		"${build_target}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static agent"
	DESTDIR="${destdir}" AGENT_POLICY=${agent_policy} "${agent_builder}"
}

install_agent() {
	install_agent_helper
}

install_agent_opa() {
	install_agent_helper "yes"
}

install_tools_helper() {
	tool=${1}

	latest_artefact="$(git log -1 --pretty=format:"%h" ${repo_root_dir}/src/tools/${tool})"
	latest_builder_image="$(get_tools_image_name)"

	install_cached_tarball_component \
		"${tool}" \
		"${latest_artefact}" \
		"${latest_builder_image}" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0


	info "build static ${tool}"
	${tools_builder} ${tool}

	tool_binary=${tool}
	[ ${tool} = "agent-ctl" ] && tool_binary="kata-agent-ctl"
	[ ${tool} = "trace-forwarder" ] && tool_binary="kata-trace-forwarder"
	binary=$(find ${repo_root_dir}/src/tools/${tool}/ -type f -name ${tool_binary})

	info "Install static ${tool_binary}"
	mkdir -p "${destdir}/opt/kata/bin/"
	sudo install -D --owner root --group root --mode 0744 ${binary} "${destdir}/opt/kata/bin/${tool_binary}"
}

install_agent_ctl() {
	install_tools_helper "agent-ctl"
}

install_kata_ctl() {
	install_tools_helper "kata-ctl"
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
		install_initrd
		install_initrd_mariner
		install_initrd_sev
		install_kata_ctl
		install_kernel
		install_kernel_dragonball_experimental
		install_kernel_tdx_experimental
		install_log_parser_rs
		install_nydus
		install_ovmf
		install_ovmf_sev
		install_qemu
		install_qemu_snp_experimental
		install_qemu_tdx_experimental
		install_stratovirt
		install_runk
		install_shimv2
		install_tdvf
		install_trace_forwarder
		install_virtiofsd
		;;

	agent) install_agent ;;

	agent-opa) install_agent_opa ;;

	agent-ctl) install_agent_ctl ;;
	
	boot-image-se) install_se_image ;;

	cloud-hypervisor) install_clh ;;

	cloud-hypervisor-glibc) install_clh_glibc ;;

	firecracker) install_firecracker ;;

	kata-ctl) install_kata_ctl ;;

	kernel) install_kernel ;;

	kernel-dragonball-experimental) install_kernel_dragonball_experimental ;;

	kernel-nvidia-gpu) install_kernel_nvidia_gpu ;;

	kernel-nvidia-gpu-snp) install_kernel_nvidia_gpu_snp;;

	kernel-nvidia-gpu-tdx-experimental) install_kernel_nvidia_gpu_tdx_experimental;;

	kernel-tdx-experimental) install_kernel_tdx_experimental ;;

	kernel-sev) install_kernel_sev ;;

	nydus) install_nydus ;;

	ovmf) install_ovmf ;;

	ovmf-sev) install_ovmf_sev ;;

	qemu) install_qemu ;;

	qemu-snp-experimental) install_qemu_snp_experimental ;;

	qemu-tdx-experimental) install_qemu_tdx_experimental ;;

	stratovirt) install_stratovirt ;;

	rootfs-image) install_image ;;

	rootfs-image-tdx) install_image_tdx ;;

	rootfs-initrd) install_initrd ;;

	rootfs-initrd-mariner) install_initrd_mariner ;;

	rootfs-initrd-sev) install_initrd_sev ;;

	runk) install_runk ;;
	
	shim-v2) install_shimv2 ;;

	tdvf) install_tdvf ;;

	trace-forwarder) install_trace_forwarder ;;

	virtiofsd) install_virtiofsd ;;

	*)
		die "Invalid build target ${build_target}"
		;;
	esac

	if [ ! -f "${final_tarball_path}" ]; then
		cd "${destdir}"
		sudo tar cvfJ "${final_tarball_path}" "."
	fi
	tar tvf "${final_tarball_path}"

	pushd ${workdir}
	echo "${latest_artefact}" > ${build_target}-version
	echo "${latest_builder_image}" > ${build_target}-builder-image-version
	sha256sum "${final_tarball_name}" > ${build_target}-sha256sum

	if [ "${PUSH_TO_REGISTRY}" = "yes" ]; then
		if [ -z "${ARTEFACT_REGISTRY}" ] ||
			[ -z "${ARTEFACT_REGISTRY_USERNAME}" ] ||
			[ -z "${ARTEFACT_REGISTRY_PASSWORD}" ] ||
		      	[ -z "${TARGET_BRANCH}" ]; then
			die "ARTEFACT_REGISTRY, ARTEFACT_REGISTRY_USERNAME, ARTEFACT_REGISTRY_PASSWORD and TARGET_BRANCH must be passed to the script when pushing the artefacts to the registry!"
		fi

		echo "${ARTEFACT_REGISTRY_PASSWORD}" | sudo oras login "${ARTEFACT_REGISTRY}" -u "${ARTEFACT_REGISTRY_USERNAME}" --password-stdin

		sudo oras push ${ARTEFACT_REGISTRY}/kata-containers/cached-artefacts/${build_target}:latest-${TARGET_BRANCH}-$(uname -m) ${final_tarball_name} ${build_target}-version ${build_target}-builder-image-version ${build_target}-sha256sum
		sudo oras logout "${ARTEFACT_REGISTRY}"
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
		agent-opa
		agent-ctl
		cloud-hypervisor
		firecracker
		kata-ctl
		kernel
		kernel-experimental
		nydus
		qemu
		stratovirt
		rootfs-image
		rootfs-initrd
		rootfs-initrd-mariner
		runk
		shim-v2
		trace-forwarder
		virtiofsd
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
