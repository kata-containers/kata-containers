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
readonly qemu_experimental_builder="${static_build_dir}/qemu/build-static-qemu-experimental.sh"
readonly shimv2_builder="${static_build_dir}/shim-v2/build.sh"
readonly td_shim_builder="${static_build_dir}/td-shim/build.sh"
readonly virtiofsd_builder="${static_build_dir}/virtiofsd/build.sh"
readonly nydus_builder="${static_build_dir}/nydus/build.sh"

readonly rootfs_builder="${repo_root_dir}/tools/packaging/guest-image/build_image.sh"
readonly se_image_builder="${repo_root_dir}/tools/packaging/guest-image/build_se_image.sh"

readonly cc_prefix="/opt/confidential-containers"
readonly qemu_cc_builder="${static_build_dir}/qemu/build-static-qemu-cc.sh"

source "${script_dir}/../../scripts/lib.sh"

readonly jenkins_url="http://jenkins.katacontainers.io"
readonly cached_artifacts_path="lastSuccessfulBuild/artifact/artifacts"

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
	rootfs-image
	rootfs-initrd
	rootfs-initrd-sev
	shim-v2
	tdvf
	virtiofsd
	cc
	cc-cloud-hypervisor
	cc-kernel
	cc-tdx-kernel
	cc-sev-kernel
	cc-qemu
	cc-snp-qemu
	cc-tdx-qemu
	cc-rootfs-image
	cc-rootfs-initrd
	cc-sev-rootfs-initrd
	cc-se-image
	cc-shimv2
	cc-virtiofsd
	cc-sev-ovmf
	cc-x86_64-ovmf
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
	wget "${jenkins_build_url}/${component_tarball_name}" || return $(cleanup_and_fail)
	wget "${jenkins_build_url}/sha256sum-${component_tarball_name}" || return $(cleanup_and_fail)
	sha256sum -c "sha256sum-${component_tarball_name}" || return $(cleanup_and_fail)
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
	local initramfs_last_commit=""
	if [ -n "${tee}" ]; then
		if [ "${tee}" == "tdx" ]; then
			jenkins="${jenkins_url}/job/kata-containers-2.0-rootfs-image-${tee}-cc-$(uname -m)/${cached_artifacts_path}"
			component="${tee}-rootfs-image"
			root_hash_vanilla=""
			root_hash_tdx="root_hash_${tee}.txt"
		fi
		if [ "${tee}" == "sev" ]; then
			jenkins="${jenkins_url}/job/kata-containers-2.0-rootfs-initrd-${tee}-cc-$(uname -m)/${cached_artifacts_path}"
			component="${tee}-rootfs-initrd"
			root_hash_vanilla=""
			initramfs_last_commit="$(get_initramfs_image_name)"
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
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"

	install_cached_component \
		"${component}" \
		"${jenkins}" \
		"${osbuilder_last_commit}-${guest_image_last_commit}-${initramfs_last_commit}-${agent_last_commit}-${libs_last_commit}-${attestation_agent_version}-${gperf_version}-${libseccomp_version}-${pause_version}-${rust_version}-${image_type}-${AA_KBC}" \
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
	install_cc_image "${AA_KBC}" "${image_type}" "sev" "" "sev"
}

install_cc_se_image() {
	info "Create IBM SE image configured with AA_KBC=${AA_KBC}"
	"${se_image_builder}" --destdir="${destdir}"
}

install_cc_tdx_image() {
	AA_KBC="cc_kbc_tdx"
	image_type="image"
	image_suffix="tdx"
	root_hash_suffix="tdx"
	install_cc_image "${AA_KBC}" "${image_type}" "${image_suffix}" "${root_hash_suffix}" "tdx"
}

#Install CC kernel asset
install_cc_kernel() {
	export KATA_BUILD_CC=yes
	export kernel_version="$(yq r $versions_yaml assets.kernel.version)"

	local kernel_kata_config_version="$(cat ${repo_root_dir}/tools/packaging/kernel/kata_config_version)"

	install_cached_component \
		"kernel" \
		"${jenkins_url}/job/kata-containers-2.0-kernel-cc-$(uname -m)/${cached_artifacts_path}" \
		"${kernel_version}-${kernel_kata_config_version}" \
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
	local runtime_rs_last_commit="$(get_last_modification "${repo_root_dir}/src/runtime-rs")"
	local protocols_last_commit="$(get_last_modification "${repo_root_dir}/src/libs/protocols")"
	local golang_version="$(get_from_kata_deps "languages.golang.meta.newest-version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"
	local shim_v2_version="${shim_v2_last_commit}-${protocols_last_commit}-${runtime_rs_last_commit}-${golang_version}-${rust_version}"

	install_cached_cc_shim_v2 \
		"shim-v2" \
		"${jenkins_url}/job/kata-containers-2.0-shim-v2-cc-$(uname -m)/${cached_artifacts_path}" \
		"${shim_v2_version}" \
		"$(get_shim_v2_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	GO_VERSION="$(yq r ${versions_yaml} languages.golang.meta.newest-version)"
	RUST_VERSION="$(yq r ${versions_yaml} languages.rust.meta.newest-version)"
	export GO_VERSION
	export RUST_VERSION
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

# Install cached kernel compoenent
install_cached_kernel_component() {
	tee="${1}"
	kernel_version="${2}"
	module_dir="${3:-}"

	local kernel_kata_config_version="$(cat ${repo_root_dir}/tools/packaging/kernel/kata_config_version)"

	install_cached_component \
		"kernel" \
		"${jenkins_url}/job/kata-containers-2.0-kernel-${tee}-cc-$(uname -m)/${cached_artifacts_path}" \
		"${kernel_version}-${kernel_kata_config_version}" \
		"$(get_kernel_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		|| return 1

	[ "${tee}" == "tdx" ] && return 0

	# SEV specific code path
	install_cached_component \
		"kernel-modules" \
		"${jenkins_url}/job/kata-containers-2.0-kernel-sev-cc-$(uname -m)/${cached_artifacts_path}" \
		"${kernel_version}" \
		"$(get_kernel_image_name)" \
		"kata-static-cc-sev-kernel-modules.tar.xz" \
		"${workdir}/kata-static-cc-sev-kernel-modules.tar.xz" \
	|| return 1

	mkdir -p "${module_dir}"
	tar xvf "${workdir}/kata-static-cc-sev-kernel-modules.tar.xz" -C  "${module_dir}" && return 0

	return 1
}

#Install CC kernel assert, with TEE support
install_cc_tee_kernel() {
	export KATA_BUILD_CC=yes
	tee="${1}"
	kernel_version="${2}"
	module_dir="${3:-}"

	[[ "${tee}" != "tdx" && "${tee}" != "sev" ]] && die "Non supported TEE"

	export kernel_version=${kernel_version}

	install_cached_kernel_component "${tee}" "${kernel_version}" "${module_dir}" && return 0

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
	default_patches_dir="${repo_root_dir}/tools/packaging/kernel/patches"
	module_dir="${repo_root_dir}/tools/packaging/kata-deploy/local-build/build/cc-sev-kernel/builddir/kata-linux-${kernel_version#v}-$(get_config_version)/lib/modules/${kernel_version#v}"
	install_cc_tee_kernel "sev" "${kernel_version}" "${module_dir}"
}

install_cc_tee_qemu() {
	tee="${1}"

	[[ "${tee}" != "tdx" && "${tee}" != "snp" ]] && die "Non supported TEE"

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

install_cc_snp_qemu() {
	install_cc_tee_qemu "snp"
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

install_cc_x86_64_ovmf(){
 	install_cc_tee_ovmf "x86_64" "edk2-x86_64.tar.gz"
}

#Install guest image
install_image() {
	local jenkins="${jenkins_url}/job/kata-containers-main-rootfs-image-$(uname -m)/${cached_artifacts_path}"
	local component="rootfs-image"

	local osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local agent_last_commit="$(get_last_modification "${repo_root_dir}/src/agent")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version="$(get_from_kata_deps "externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps "externals.libseccomp.version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"

	install_cached_component \
		"${component}" \
		"${jenkins}" \
		"${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-image" \
		"" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "Create image"
	"${rootfs_builder}" --imagetype=image --prefix="${prefix}" --destdir="${destdir}"
}

#Install guest initrd
install_initrd() {
	local initrd_type="${1:-"initrd"}"
	local initrd_suffix="${2:-""}"
	local jenkins="${jenkins_url}/job/kata-containers-main-rootfs-${initrd_type}-$(uname -m)/${cached_artifacts_path}"
	local component="rootfs-${initrd_type}"

	local osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local agent_last_commit="$(get_last_modification "${repo_root_dir}/src/agent")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version="$(get_from_kata_deps "externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps "externals.libseccomp.version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"

	install_cached_component \
		"${component}" \
		"${jenkins}" \
		"${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-${initrd_type}" \
		"" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "Create initrd"
	"${rootfs_builder}" --imagetype=initrd --prefix="${prefix}" --destdir="${destdir}" --image_initrd_suffix="${initrd_suffix}"
}

#Install guest initrd for sev
install_initrd_sev() {
	install_initrd "initrd-sev" "sev"
}

#Install kernel component helper
install_cached_kernel_tarball_component() {
	local kernel_name=${1}

	install_cached_tarball_component \
		"${kernel_name}" \
		"${jenkins_url}/job/kata-containers-main-${kernel_name}-$(uname -m)/${cached_artifacts_path}" \
		"${kernel_version}-${kernel_kata_config_version}" \
		"$(get_kernel_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		|| return 1
	
	if [[ "${kernel_name}" != "kernel-sev" ]]; then
		return 0
	fi

	# SEV specific code path
	install_cached_tarball_component \
		"${kernel_name}" \
		"${jenkins_url}/job/kata-containers-main-${kernel_name}-$(uname -m)/${cached_artifacts_path}" \
		"${kernel_version}-${kernel_kata_config_version}" \
		"$(get_kernel_image_name)" \
		"kata-static-kernel-sev-modules.tar.xz" \
		"${workdir}/kata-static-kernel-sev-modules.tar.xz" \
		|| return 1

	mkdir -p "${module_dir}"
	tar xvf "${workdir}/kata-static-kernel-sev-modules.tar.xz" -C  "${module_dir}" && return 0

	return 1
}

install_cc_initrd() {
	info "Create initrd"
	"${rootfs_builder}" --imagetype=initrd --prefix="${cc_prefix}" --destdir="${destdir}"
}

#Install kernel asset
install_kernel_helper() {
	local kernel_version_yaml_path="${1}"
	local kernel_name="${2}"
	local extra_cmd=${3}

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
	local kernel_url="$(get_from_kata_deps assets.kernel.snp.url)"

	install_kernel_helper \
		"assets.kernel.snp.version" \
		"kernel-nvidia-gpu-snp" \
		"-x snp -g nvidia -u ${kernel_url} -H deb"
}

#Install GPU and TDX experimental enabled kernel asset
install_kernel_nvidia_gpu_tdx_experimental() {
	local kernel_url="$(get_from_kata_deps assets.kernel-tdx-experimental.url)"

	install_kernel_helper \
		"assets.kernel-tdx-experimental.version" \
		"kernel-nvidia-gpu-tdx-experimental" \
		"-x tdx -g nvidia -u ${kernel_url} -H deb"
}

#Install experimental kernel asset
install_kernel_experimental() {
	install_kernel_helper \
		"assets.kernel-experimental.version" \
		"kernel-experimental" \
		"-f -b experimental"
}

#Install experimental TDX kernel asset
install_kernel_tdx_experimental() {
	local kernel_url="$(get_from_kata_deps assets.kernel-tdx-experimental.url)"

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

	install_cached_tarball_component \
		"${qemu_name}" \
		"${jenkins_url}/job/kata-containers-main-${qemu_name}-$(uname -m)/${cached_artifacts_path}" \
		"${qemu_version}-$(calc_qemu_files_sha256sum)" \
		"$(get_qemu_image_name)" \
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

	install_cached_tarball_component \
		"firecracker" \
		"${jenkins_url}/job/kata-containers-main-firecracker-$(uname -m)/${cached_artifacts_path}" \
		"${firecracker_version}" \
		"" \
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

# Install static cloud-hypervisor asset
install_clh() {
	install_cached_component \
		"cloud-hypervisor" \
		"${jenkins_url}/job/kata-containers-main-clh-$(uname -m)/${cached_artifacts_path}" \
		"$(get_from_kata_deps "assets.hypervisor.cloud_hypervisor.version")" \
		"" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

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
	install_cached_component \
		"virtiofsd" \
		"${jenkins_url}/job/kata-containers-main-virtiofsd-$(uname -m)/${cached_artifacts_path}" \
		"$(get_from_kata_deps "externals.virtiofsd.version")-$(get_from_kata_deps "externals.virtiofsd.toolchain")" \
		"$(get_virtiofsd_image_name)" \
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

	install_cached_tarball_component \
		"nydus" \
		"${jenkins_url}/job/kata-containers-main-nydus-$(uname -m)/${cached_artifacts_path}" \
		"$(get_from_kata_deps "externals.nydus.version")" \
		"" \
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
	local shim_v2_version="${shim_v2_last_commit}-${protocols_last_commit}-${runtime_rs_last_commit}-${GO_VERSION}-${RUST_VERSION}"

	install_cached_component \
		"shim-v2" \
		"${jenkins_url}/job/kata-containers-main-shim-v2-$(uname -m)/${cached_artifacts_path}" \
		"${shim_v2_version}" \
		"$(get_shim_v2_image_name)" \
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
	local component_version="$(get_from_kata_deps "externals.ovmf.${ovmf_type}.version")"
	[ "${ovmf_type}" == "tdx" ] && component_name="tdvf"
	install_cached_tarball_component \
		"${component_name}" \
		"${jenkins_url}/job/kata-containers-main-ovmf-${ovmf_type}-$(uname -m)/${cached_artifacts_path}" \
		"${component_version}" \
		"$(get_ovmf_image_name)" \
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
		install_initrd_sev
		install_kernel
		install_kernel_dragonball_experimental
		install_kernel_tdx_experimental
		install_nydus
		install_ovmf
		install_ovmf_sev
		install_qemu
		install_qemu_snp_experimental
		install_qemu_tdx_experimental
		install_shimv2
		install_tdvf
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

	cc-snp-qemu) install_cc_snp_qemu ;;

	cc-rootfs-image) install_cc_image ;;

	cc-rootfs-initrd) install_cc_initrd ;;

	cc-sev-rootfs-initrd) install_cc_sev_image ;;

	cc-se-image) install_cc_se_image ;;

	cc-tdx-rootfs-image) install_cc_tdx_image ;;

	cc-shim-v2) install_cc_shimv2 ;;

	cc-virtiofsd) install_cc_virtiofsd ;;

	cc-tdx-kernel) install_cc_tdx_kernel ;;

	cc-sev-kernel) install_cc_sev_kernel ;;

	cc-tdx-qemu) install_cc_tdx_qemu ;;

	cc-tdx-td-shim) install_cc_tdx_td_shim ;;

	cc-tdx-tdvf) install_cc_tdx_tdvf ;;

	cc-sev-ovmf) install_cc_sev_ovmf ;;

	cc-x86_64-ovmf) install_cc_x86_64_ovmf ;;

	cloud-hypervisor) install_clh ;;

	firecracker) install_firecracker ;;

	kernel) install_kernel ;;

	kernel-dragonball-experimental) install_kernel_dragonball_experimental ;;

	kernel-experimental) install_kernel_experimental ;;

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

	rootfs-image) install_image ;;

	rootfs-initrd) install_initrd ;;

	rootfs-initrd-sev) install_initrd_sev ;;
	
	shim-v2) install_shimv2 ;;

	tdvf) install_tdvf ;;

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
