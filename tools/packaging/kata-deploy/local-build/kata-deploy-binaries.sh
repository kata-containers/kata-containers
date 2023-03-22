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
readonly kernel_builder="${static_build_dir}/kernel/build.sh"
readonly ovmf_builder="${static_build_dir}/ovmf/build.sh"
readonly qemu_builder="${static_build_dir}/qemu/build-static-qemu.sh"
readonly qemu_experimental_builder="${static_build_dir}/qemu/build-static-qemu-experimental.sh"
readonly shimv2_builder="${static_build_dir}/shim-v2/build.sh"
readonly virtiofsd_builder="${static_build_dir}/virtiofsd/build.sh"
readonly nydus_builder="${static_build_dir}/nydus/build.sh"

readonly rootfs_builder="${repo_root_dir}/tools/packaging/guest-image/build_image.sh"

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
	kernel-tdx-experimental
	nydus
	qemu
	qemu-tdx-experimental
	rootfs-image
	rootfs-initrd
	shim-v2
	tdvf
	virtiofsd
EOF

	exit "${return_code}"
}


cleanup_and_fail() {
	rm -f "${component_tarball_path}"
	return 1
}

install_cached_tarball_component() {
	local component="${1}"
	local jenkins_build_url="${2}"
	local current_version="${3}"
	local current_image_version="${4}"
	local component_tarball_name="${5}"
	local component_tarball_path="${6}"

	local cached_version=$(curl -sfL "${jenkins_build_url}/latest" | awk '{print $1}') || cached_version="none"
	local cached_image_version=$(curl -sfL "${jenkins_build_url}/latest_image" | awk '{print $1}') || cached_image_version="none"

	[ "${cached_image_version}" != "${current_image_version}" ] && return 1
	[ "${cached_version}" != "${current_version}" ] && return 1

	info "Using cached tarball of ${component}"
	echo "Downloading tarball from: ${jenkins_build_url}/${component_tarball_name}"
	wget "${jenkins_build_url}/${component_tarball_name}" || return $(cleanup_and_fail)
	wget "${jenkins_build_url}/sha256sum-${component_tarball_name}" || return $(cleanup_and_fail)
	sha256sum -c "sha256sum-${component_tarball_name}" || return $(cleanup_and_fail)
	mv "${component_tarball_name}" "${component_tarball_path}"
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

	install_cached_tarball_component \
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
	local jenkins="${jenkins_url}/job/kata-containers-main-rootfs-initrd-$(uname -m)/${cached_artifacts_path}"
	local component="rootfs-initrd"

	local osbuilder_last_commit="$(get_last_modification "${repo_root_dir}/tools/osbuilder")"
	local guest_image_last_commit="$(get_last_modification "${repo_root_dir}/tools/packaging/guest-image")"
	local agent_last_commit="$(get_last_modification "${repo_root_dir}/src/agent")"
	local libs_last_commit="$(get_last_modification "${repo_root_dir}/src/libs")"
	local gperf_version="$(get_from_kata_deps "externals.gperf.version")"
	local libseccomp_version="$(get_from_kata_deps "externals.libseccomp.version")"
	local rust_version="$(get_from_kata_deps "languages.rust.meta.newest-version")"

	install_cached_tarball_component \
		"${component}" \
		"${jenkins}" \
		"${osbuilder_last_commit}-${guest_image_last_commit}-${agent_last_commit}-${libs_last_commit}-${gperf_version}-${libseccomp_version}-${rust_version}-initrd" \
		"" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "Create initrd"
	"${rootfs_builder}" --imagetype=initrd --prefix="${prefix}" --destdir="${destdir}"
}

#Install kernel asset
install_kernel_helper() {
	local kernel_version_yaml_path="${1}"
	local kernel_name="${2}"
	local extra_cmd=${3}

	export kernel_version="$(get_from_kata_deps ${kernel_version_yaml_path})"
	local kernel_kata_config_version="$(cat ${repo_root_dir}/tools/packaging/kernel/kata_config_version)"

	install_cached_tarball_component \
		"${kernel_name}" \
		"${jenkins_url}/job/kata-containers-main-${kernel_name}-$(uname -m)/${cached_artifacts_path}" \
		"${kernel_version}-${kernel_kata_config_version}" \
		"$(get_kernel_image_name)" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

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

# Install static firecracker asset
install_firecracker() {
	install_cached_tarball_component \
		"firecracker" \
		"${jenkins_url}/job/kata-containers-main-firecracker-$(uname -m)/${cached_artifacts_path}" \
		"$(get_from_kata_deps "assets.hypervisor.firecracker.version")" \
		"" \
		"${final_tarball_name}" \
		"${final_tarball_path}" \
		&& return 0

	info "build static firecracker"
	"${firecracker_builder}"
	info "Install static firecracker"
	mkdir -p "${destdir}/opt/kata/bin/"
	sudo install -D --owner root --group root --mode 0744 firecracker/firecracker-static "${destdir}/opt/kata/bin/firecracker"
	sudo install -D --owner root --group root --mode 0744 firecracker/jailer-static "${destdir}/opt/kata/bin/jailer"
}

# Install static cloud-hypervisor asset
install_clh() {
	install_cached_tarball_component \
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
	install_cached_tarball_component \
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

	install_cached_tarball_component \
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
	tarball_name="${2:-edk2.tar.xz}"

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
		install_kernel_dragonball_experimental
		install_kernel_tdx_experimental
		install_nydus
		install_qemu
		install_qemu_tdx_experimental
		install_shimv2
		install_tdvf
		install_virtiofsd
		;;

	cloud-hypervisor) install_clh ;;

	firecracker) install_firecracker ;;

	kernel) install_kernel ;;

	nydus) install_nydus ;;

	kernel-dragonball-experimental) install_kernel_dragonball_experimental ;;

	kernel-experimental) install_kernel_experimental ;;

	kernel-tdx-experimental) install_kernel_tdx_experimental ;;

	qemu) install_qemu ;;

	qemu-tdx-experimental) install_qemu_tdx_experimental ;;

	rootfs-image) install_image ;;

	rootfs-initrd) install_initrd ;;

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
