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

readonly clh_builder="${static_build_dir}/cloud-hypervisor/build-static-clh.sh"
readonly firecracker_builder="${static_build_dir}/firecracker/build-static-firecracker.sh"
readonly initramfs_builder="${static_build_dir}/initramfs/build.sh"
readonly kernel_builder="${static_build_dir}/kernel/build.sh"
readonly ovmf_builder="${static_build_dir}/ovmf/build.sh"
readonly qemu_builder="${static_build_dir}/qemu/build-static-qemu.sh"
readonly shimv2_builder="${static_build_dir}/shim-v2/build.sh"
readonly td_shim_builder="${static_build_dir}/td-shim/build.sh"
readonly virtiofsd_builder="${static_build_dir}/virtiofsd/build.sh"
readonly nydus_builder="${static_build_dir}/nydus/build.sh"

readonly rootfs_builder="${repo_root_dir}/tools/packaging/guest-image/build_image.sh"

readonly cc_prefix="/opt/confidential-containers"
readonly qemu_cc_builder="${static_build_dir}/qemu/build-static-qemu-cc.sh"

source "${script_dir}/../../scripts/lib.sh"

ARCH=$(uname -m)

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
	cloud-hypervisor
	firecracker
	kernel
	kernel-experimental
	nydus
	qemu
	rootfs-image
	rootfs-initrd
	shim-v2
	virtiofsd
	cc
	cc-cloud-hypervisor
	cc-kernel
	cc-tdx-kernel
	cc-sev-kernel
	cc-qemu
	cc-tdx-qemu
	cc-rootfs-image
	cc-sev-rootfs-initrd
	cc-shimv2
	cc-virtiofsd
	cc-sev-ovmf
EOF

	exit "${return_code}"
}

cleanup_and_fail() {
	rm -f "${component_tarball_path}"
	return 1
}

install_cached_component() {
	local component="${1}"
	local jenkins_build_url="${2}"
	local current_version="${3}"
	local current_image_version="${4}"
	local component_tarball_name="${5}"
	local component_tarball_path="${6}"
	local root_hash_vanilla="${7:-""}"
	local root_hash_tdx="${8:-""}"

	local cached_version=$(curl -sfL "${jenkins_build_url}/latest" | awk '{print $1}') || cached_version="none"
	local cached_image_version=$(curl -sfL "${jenkins_build_url}/latest_image" | awk '{print $1}') || cached_image_version="none"

	[ "${cached_image_version}" != "${current_image_version}" ] && return 1
	[ "${cached_version}" != "${current_version}" ] && return 1

	info "Using cached tarball of ${component}"
	echo "Downloading tarball from: ${jenkins_build_url}/${component_tarball_name}"
	wget "${jenkins_build_url}/${component_tarball_name}" || return cleanup_and_fail
	wget "${jenkins_build_url}/sha256sum-${component_tarball_name}" || return cleanup_and_fail
	sha256sum -c "sha256sum-${component_tarball_name}" || return cleanup_and_fail
	if [ -n "${root_hash_vanilla}" ]; then
		wget "${jenkins_build_url}/${root_hash_vanilla}" || return cleanup_and_fail
		mv "${root_hash_vanilla}" "${repo_root_dir}/tools/osbuilder/"
	fi
	if [ -n "${root_hash_tdx}" ]; then
		wget "${jenkins_build_url}/${root_hash_tdx}" || return cleanup_and_fail
		mv "${root_hash_tdx}" "${repo_root_dir}/tools/osbuilder/"
	fi
	mv "${component_tarball_name}" "${component_tarball_path}"
}

# We've to add a different cached function here as for using the shim-v2 caching
# we have to rely and check some artefacts coming from the cc-rootfs-image and the
# cc-tdx-rootfs-image jobs.
install_cached_cc_shim_v2() {
	local component="${1}"
	local jenkins_build_url="${2}"
	local current_version="${3}"
	local current_image_version="${4}"
	local component_tarball_name="${5}"
	local component_tarball_path="${6}"
	local root_hash_vanilla="${repo_root_dir}/tools/osbuilder/root_hash_vanilla.txt"
	local root_hash_tdx="${repo_root_dir}/tools/osbuilder/root_hash_tdx.txt"

	local rootfs_image_cached_root_hash="${jenkins_url}/job/kata-containers-2.0-rootfs-image-cc-$(uname -m)/${cached_artifacts_path}/root_hash_vanilla.txt"
	local tdx_rootfs_image_cached_root_hash="${jenkins_url}/job/kata-containers-2.0-rootfs-image-tdx-cc-$(uname -m)/${cached_artifacts_path}/root_hash_tdx.txt"


	wget "${rootfs_image_cached_root_hash}" -O "rootfs_root_hash_vanilla.txt" || return 1
	if [ -f "${root_hash_vanilla}" ]; then
		# There's already a pre-existent root_hash_vanilla.txt,
		# let's check whether this is the same one cached on the
		# rootfs job.

		# In case it's not the same, let's proceed building the
		# shim-v2 with what we have locally.
		diff "${root_hash_vanilla}" "rootfs_root_hash_vanilla.txt" > /dev/null || return 1
	fi
	mv "rootfs_root_hash_vanilla.txt" "${root_hash_vanilla}"

	wget "${rootfs_image_cached_root_hash}" -O "rootfs_root_hash_tdx.txt" || return 1
	if [ -f "${root_hash_tdx}" ]; then
		# There's already a pre-existent root_hash_tdx.txt,
		# let's check whether this is the same one cached on the
		# rootfs job.

		# In case it's not the same, let's proceed building the
		# shim-v2 with what we have locally.
		diff "${root_hash_tdx}" "rootfs_root_hash_tdx.txt" > /dev/null || return 1
	fi
	mv "rootfs_root_hash_tdx.txt" "${root_hash_tdx}"

	wget "${jenkins_build_url}/root_hash_vanilla.txt" -O "shim_v2_root_hash_vanilla.txt" || return 1
	diff "${root_hash_vanilla}" "shim_v2_root_hash_vanilla.txt" > /dev/null || return 1

	wget "${jenkins_build_url}/root_hash_tdx.txt" -O "shim_v2_root_hash_tdx.txt" || return 1
	diff "${root_hash_tdx}" "shim_v2_root_hash_tdx.txt" > /dev/null || return 1

	install_cached_component \
		"${component}" \
		"${jenkins_build_url}" \
		"${current_version}" \
		"${current_image_version}" \
		"${component_tarball_name}" \
		"${component_tarball_path}" \
		"$(basename ${root_hash_vanilla})" \
		"$(basename ${root_hash_tdx})"
}

# Install static CC cloud-hypervisor asset
install_cc_clh() {
	install_cached_component \
		"cloud-hypervisor" \
		"${jenkins_url}/job/kata-containers-2.0-clh-cc-$(uname -m)/${cached_artifacts_path}" \
		"$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version")" \
		"" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	if [[ "${ARCH}" == "x86_64" ]]; then
		export features="tdx"
	fi

	info "build static CC cloud-hypervisor"
	"${clh_builder}"
	info "Install static CC cloud-hypervisor"
	mkdir -p "${destdir}/${cc_prefix}/bin/"
	sudo install -D --owner root --group root --mode 0744 cloud-hypervisor/cloud-hypervisor "${destdir}/${cc_prefix}/bin/cloud-hypervisor"
}

#Install cc capable guest image
install_cc_image() {
	export AA_KBC="${1:-offline_fs_kbc}"
	image_type="${2:-image}"
	image_initrd_suffix="${3:-""}"
	root_hash_suffix="${4:-""}"
	tee="${5:-""}"
	export KATA_BUILD_CC=yes

	local jenkins="${jenkins_url}/job/kata-containers-2.0-rootfs-image-cc-$(uname -m)/${cached_artifacts_path}"
	local component="rootfs-image"
	local root_hash_vanilla="root_hash_vanilla.txt"
	local root_hash_tdx=""
	if [ -n "${tee}" ]; then
		if [ "${tee}" == "tdx" ]; then
			jenkins="${jenkins_url}/job/kata-containers-2.0-rootfs-image-${tee}-cc-$(uname -m)/${cached_artifacts_path}"
			component="${tee}-rootfs-image"
			root_hash_vanilla=""
			root_hash_tdx="root_hash_${tee}.txt"
		fi
	fi

	local osbuilder_last_commit="$(echo $(get_last_modification "${repo_root_dir}/tools/osbuilder") | sed s/-dirty//)"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local agent_last_commit="$(get_last_modification "${repo_root_dir}/src/agent")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local attestation_agent_version="$(get_from_kata_deps "externals.attestation-agent.version")"
	local gperf_version="$(get_from_kata_deps "externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps "externals.libseccomp.version")"
	local pause_version="$(get_from_kata_deps "externals.pause.version")"
	local skopeo_version="$(get_from_kata_deps "externals.skopeo.branch")"
	local umoci_version="$(get_from_kata_deps "externals.umoci.tag")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"

	install_cached_component \
		"${component}" \
		"${jenkins}" \
		"${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${attestation_agent_version}-${gperf_version}-${libseccomp_version}-${pause_version}-${skopeo_version}-${umoci_version}-${rust_version}-${image_type}-${AA_KBC}" \
		"" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		"${root_hash_vanilla}" \
		"${root_hash_tdx}" \
		&& return 0

	info "Create CC image configured with AA_KBC=${AA_KBC}"
	"${rootfs_builder}" \
		--imagetype="${image_type}" \
		--prefix="${cc_prefix}" \
		--destdir="${destdir}" \
		--image_initrd_suffix="${image_initrd_suffix}" \
		--root_hash_suffix="${root_hash_suffix}"
}

install_cc_sev_image() {
	AA_KBC="online_sev_kbc"
	image_type="initrd"
	install_cc_image "${AA_KBC}" "${image_type}" "sev"
}

install_cc_tdx_image() {
	AA_KBC="eaa_kbc"
	image_type="image"
	image_suffix="tdx"
	root_hash_suffix="tdx"
	install_cc_image "${AA_KBC}" "${image_type}" "${image_suffix}" "${root_hash_suffix}" "tdx"
}

#Install CC kernel asset
install_cc_kernel() {
	export KATA_BUILD_CC=yes
	export kernel_version="$(yq r $versions_yaml assets.kernel.version)"

	install_cached_component \
		"kernel" \
		"${jenkins_url}/job/kata-containers-2.0-kernel-cc-$(uname -m)/${cached_artifacts_path}" \
		"${kernel_version}" \
		"$(get_kernel_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build initramfs for CC kernel"
	"${initramfs_builder}"
	DESTDIR="${destdir}" PREFIX="${cc_prefix}" "${kernel_builder}" -f -v "${kernel_version}"
}

# Install static CC qemu asset
install_cc_qemu() {
	info "build static CC qemu"
	export qemu_repo="$(yq r $versions_yaml assets.hypervisor.qemu.url)"
	export qemu_version="$(yq r $versions_yaml assets.hypervisor.qemu.version)"

	install_cached_component \
		"QEMU" \
		"${jenkins_url}/job/kata-containers-2.0-qemu-cc-$(uname -m)/${cached_artifacts_path}" \
		"${qemu_version}-$(calc_qemu_files_sha256sum)" \
		"$(get_qemu_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	"${qemu_cc_builder}"
	tar xvf "${builddir}/kata-static-qemu-cc.tar.gz" -C "${destdir}"
}

#Install all components that are not assets
install_cc_shimv2() {
	local shim_v2_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime")"
	local golang_version="$(get_from_kata_deps "languages.golang.meta.newest-version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"
	local shim_v2_version="${shim_v2_last_commit}-${golang_version}-${rust_version}"

	install_cached_cc_shim_v2 \
		"shim-v2" \
		"${jenkins_url}/job/kata-containers-2.0-shim-v2-cc-$(uname -m)/${cached_artifacts_path}" \
		"${shim_v2_version}" \
		"$(get_shim_v2_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	GO_VERSION="$(yq r ${versions_yaml} languages.golang.meta.newest-version)"
	export GO_VERSION
	export REMOVE_VMM_CONFIGS="acrn fc"

        extra_opts="DEFSERVICEOFFLOAD=true"
	if [ -f "${repo_root_dir}/tools/osbuilder/root_hash_vanilla.txt" ]; then
		root_hash=$(sudo sed -e 's/Root hash:\s*//g;t;d' "${repo_root_dir}/tools/osbuilder/root_hash_vanilla.txt")
		root_measure_config="cc_rootfs_verity.scheme=dm-verity cc_rootfs_verity.hash=${root_hash}"
		extra_opts+=" ROOTMEASURECONFIG=\"${root_measure_config}\""
	fi

	if [ -f "${repo_root_dir}/tools/osbuilder/root_hash_tdx.txt" ]; then
		root_hash=$(sudo sed -e 's/Root hash:\s*//g;t;d' "${repo_root_dir}/tools/osbuilder/root_hash_tdx.txt")
		root_measure_config="cc_rootfs_verity.scheme=dm-verity cc_rootfs_verity.hash=${root_hash}"
		extra_opts+=" ROOTMEASURECONFIGTDX=\"${root_measure_config}\""
	fi

	info "extra_opts: ${extra_opts}"
	DESTDIR="${destdir}" PREFIX="${cc_prefix}" EXTRA_OPTS="${extra_opts}" "${shimv2_builder}"
}

# Install static CC virtiofsd asset
install_cc_virtiofsd() {
	local virtiofsd_version="$(get_from_kata_deps "externals.virtiofsd.version")-$(get_from_kata_deps "externals.virtiofsd.toolchain")"
	install_cached_component \
		"virtiofsd" \
		"${jenkins_url}/job/kata-containers-2.0-virtiofsd-cc-$(uname -m)/${cached_artifacts_path}" \
		"${virtiofsd_version}" \
		"$(get_virtiofsd_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static CC virtiofsd"
	"${virtiofsd_builder}"
	info "Install static CC virtiofsd"
	mkdir -p "${destdir}/${cc_prefix}/libexec/"
	sudo install -D --owner root --group root --mode 0744 virtiofsd/virtiofsd "${destdir}/${cc_prefix}/libexec/virtiofsd"
}

#Install CC kernel assert, with TEE support
install_cc_tee_kernel() {
	export KATA_BUILD_CC=yes
	tee="${1}"
	kernel_version="${2}"

	[[ "${tee}" != "tdx" && "${tee}" != "sev" ]] && die "Non supported TEE"

	export kernel_version=${kernel_version}

	install_cached_component \
		"kernel" \
		"${jenkins_url}/job/kata-containers-2.0-kernel-${tee}-cc-$(uname -m)/${cached_artifacts_path}" \
		"${kernel_version}" \
		"$(get_kernel_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build initramfs for TEE kernel"
	"${initramfs_builder}"
	kernel_url="$(yq r $versions_yaml assets.kernel.${tee}.url)"
	DESTDIR="${destdir}" PREFIX="${cc_prefix}" "${kernel_builder}" -x "${tee}" -v "${kernel_version}" -u "${kernel_url}"
}

#Install CC kernel assert for Intel TDX
install_cc_tdx_kernel() {
	kernel_version="$(yq r $versions_yaml assets.kernel.tdx.tag)"
	install_cc_tee_kernel "tdx" "${kernel_version}"
}

install_cc_sev_kernel() {
	kernel_version="$(yq r $versions_yaml assets.kernel.sev.version)"
	install_cc_tee_kernel "sev" "${kernel_version}"
}

install_cc_tee_qemu() {
	tee="${1}"

	[ "${tee}" != "tdx" ] && die "Non supported TEE"

	export qemu_repo="$(yq r $versions_yaml assets.hypervisor.qemu.${tee}.url)"
	export qemu_version="$(yq r $versions_yaml assets.hypervisor.qemu.${tee}.tag)"
	export tee="${tee}"

	install_cached_component \
		"QEMU ${tee}" \
		"${jenkins_url}/job/kata-containers-2.0-qemu-${tee}-cc-$(uname -m)/${cached_artifacts_path}" \
		"${qemu_version}-$(calc_qemu_files_sha256sum)" \
		"$(get_qemu_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	"${qemu_cc_builder}"
	tar xvf "${builddir}/kata-static-${tee}-qemu-cc.tar.gz" -C "${destdir}"
}

install_cc_tdx_qemu() {
	install_cc_tee_qemu "tdx"
}

install_cc_tdx_td_shim() {
	install_cached_component \
		"td-shim" \
		"${jenkins_url}/job/kata-containers-2.0-td-shim-cc-$(uname -m)/${cached_artifacts_path}" \
		"$(get_from_kata_deps "externals.td-shim.version")-$(get_from_kata_deps "externals.td-shim.toolchain")" \
		"$(get_td_shim_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	DESTDIR="${destdir}" PREFIX="${cc_prefix}" "${td_shim_builder}"
	tar xvf "${builddir}/td-shim.tar.gz" -C "${destdir}"
}

install_cc_tee_ovmf() {
	tee="${1}"
	tarball_name="${2}"

	local component_name="ovmf"
	local component_version="$(get_from_kata_deps "externals.ovmf.${tee}.version")"
	[ "${tee}" == "tdx" ] && component_name="tdvf"
	install_cached_component \
		"${component_name}" \
		"${jenkins_url}/job/kata-containers-2.0-${component_name}-cc-$(uname -m)/${cached_artifacts_path}" \
		"${component_version}" \
		"$(get_ovmf_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	DESTDIR="${destdir}" PREFIX="${cc_prefix}" ovmf_build="${tee}" "${ovmf_builder}"
	tar xvf "${builddir}/${tarball_name}" -C "${destdir}"
}

install_cc_tdx_tdvf() {
	install_cc_tee_ovmf "tdx" "edk2-staging-tdx.tar.gz"
}

install_cc_sev_ovmf(){
 	install_cc_tee_ovmf "sev" "edk2-sev.tar.gz"
}

#Install guest image
install_image() {
	info "Create image"
	"${rootfs_builder}" --imagetype=image --prefix="${prefix}" --destdir="${destdir}"
}

#Install guest initrd
install_initrd() {
	info "Create initrd"
	"${rootfs_builder}" --imagetype=initrd --prefix="${prefix}" --destdir="${destdir}"
}

#Install kernel asset
install_kernel() {
	export kernel_version="$(yq r $versions_yaml assets.kernel.version)"
	DESTDIR="${destdir}" PREFIX="${prefix}" "${kernel_builder}" -f -v "${kernel_version}"
}


#Install experimental kernel asset
install_experimental_kernel() {
	info "build experimental kernel"
	export kernel_version="$(yq r $versions_yaml assets.kernel-experimental.tag)"
	info "Kernel version ${kernel_version}"
	DESTDIR="${destdir}" PREFIX="${prefix}" "${kernel_builder}" -f -b experimental -v ${kernel_version}
}

# Install static qemu asset
install_qemu() {
	info "build static qemu"
	export qemu_repo="$(yq r $versions_yaml assets.hypervisor.qemu.url)"
	export qemu_version="$(yq r $versions_yaml assets.hypervisor.qemu.version)"
	"${qemu_builder}"
	tar xvf "${builddir}/kata-static-qemu.tar.gz" -C "${destdir}"
}

# Install static firecracker asset
install_firecracker() {
	info "build static firecracker"
	"${firecracker_builder}"
	info "Install static firecracker"
	mkdir -p "${destdir}/opt/kata/bin/"
	sudo install -D --owner root --group root --mode 0744 firecracker/firecracker-static "${destdir}/opt/kata/bin/firecracker"
	sudo install -D --owner root --group root --mode 0744 firecracker/jailer-static "${destdir}/opt/kata/bin/jailer"
}

# Install static cloud-hypervisor asset
install_clh() {
	if [[ "${ARCH}" == "x86_64" ]]; then
		export features="tdx"
	fi

	info "build static cloud-hypervisor"
	"${clh_builder}"
	info "Install static cloud-hypervisor"
	mkdir -p "${destdir}/opt/kata/bin/"
	sudo install -D --owner root --group root --mode 0744 cloud-hypervisor/cloud-hypervisor "${destdir}/opt/kata/bin/cloud-hypervisor"
}

# Install static virtiofsd asset
install_virtiofsd() {
	info "build static virtiofsd"
	"${virtiofsd_builder}"
	info "Install static virtiofsd"
	mkdir -p "${destdir}/opt/kata/libexec/"
	sudo install -D --owner root --group root --mode 0744 virtiofsd/virtiofsd "${destdir}/opt/kata/libexec/virtiofsd"
}

# Install static nydus asset
install_nydus() {
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
	GO_VERSION="$(yq r ${versions_yaml} languages.golang.meta.newest-version)"
	RUST_VERSION="$(yq r ${versions_yaml} languages.rust.meta.newest-version)"
	export GO_VERSION
	export RUST_VERSION
	DESTDIR="${destdir}" PREFIX="${prefix}" "${shimv2_builder}"
}

get_kata_version() {
	local v
	v=$(cat "${version_file}")
	echo ${v}
}

handle_build() {
	info "DESTDIR ${destdir}"
	local build_target
	build_target="$1"

	export final_tarball_path="${workdir}/kata-static-${build_target}.tar.xz"
	export final_tarball_name="$(basename ${final_tarball_path})"
	rm -f ${final_tarball_name}

	case "${build_target}" in
	all)
		install_clh
		install_firecracker
		install_image
		install_initrd
		install_kernel
		install_nydus
		install_qemu
		install_shimv2
		install_virtiofsd
		;;

	cc)
		install_cc_clh
		install_cc_kernel
		install_cc_qemu
		install_cc_image
		install_cc_shimv2
		install_cc_virtiofsd
		install_cc_sev_image
		;;

	cc-cloud-hypervisor) install_cc_clh ;;

	cc-kernel) install_cc_kernel ;;

	cc-qemu) install_cc_qemu ;;

	cc-rootfs-image) install_cc_image ;;

	cc-sev-rootfs-initrd) install_cc_sev_image ;;

	cc-tdx-rootfs-image) install_cc_tdx_image ;;

	cc-shim-v2) install_cc_shimv2 ;;

	cc-virtiofsd) install_cc_virtiofsd ;;

	cc-tdx-kernel) install_cc_tdx_kernel ;;

	cc-sev-kernel) install_cc_sev_kernel ;;

	cc-tdx-qemu) install_cc_tdx_qemu ;;

	cc-tdx-td-shim) install_cc_tdx_td_shim ;;

	cc-tdx-tdvf) install_cc_tdx_tdvf ;;

	cc-sev-ovmf) install_cc_sev_ovmf ;;

	cloud-hypervisor) install_clh ;;

	firecracker) install_firecracker ;;

	kernel) install_kernel ;;

	nydus) install_nydus ;;

	kernel-experimental) install_experimental_kernel;;

	qemu) install_qemu ;;

	rootfs-image) install_image ;;

	rootfs-initrd) install_initrd ;;

	shim-v2) install_shimv2 ;;

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
		cc-rootfs-image
		cc-shim-v2
		cloud-hypervisor
		firecracker
		kernel
		kernel-experimental
		nydus
		qemu
		rootfs-image
		rootfs-initrd
		shim-v2
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
