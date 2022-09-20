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

readonly prefix="/opt/kata"
readonly repo_root_dir="$(cd "${script_dir}/../../../.." && pwd)"
readonly static_build_dir="${repo_root_dir}/tools/packaging/static-build"
readonly version_file="${repo_root_dir}/VERSION"
readonly versions_yaml="${repo_root_dir}/versions.yaml"

readonly clh_builder="${static_build_dir}/cloud-hypervisor/build-static-clh.sh"
readonly firecracker_builder="${static_build_dir}/firecracker/build-static-firecracker.sh"
readonly kernel_builder="${static_build_dir}/kernel/build.sh"
readonly ovmf_builder="${static_build_dir}/ovmf/build.sh"
readonly qemu_builder="${static_build_dir}/qemu/build-static-qemu.sh"
readonly shimv2_builder="${static_build_dir}/shim-v2/build.sh"
readonly td_shim_builder="${static_build_dir}/td-shim/build.sh"
readonly virtiofsd_builder="${static_build_dir}/virtiofsd/build-static-virtiofsd.sh"

readonly rootfs_builder="${repo_root_dir}/tools/packaging/guest-image/build_image.sh"

readonly cc_prefix="/opt/confidential-containers"
readonly qemu_cc_builder="${static_build_dir}/qemu/build-static-qemu-cc.sh"

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

# Install static CC cloud-hypervisor asset
install_cc_clh() {
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
	export KATA_BUILD_CC=yes

	info "Create CC image configured with AA_KBC=${AA_KBC}"
	"${rootfs_builder}" --imagetype="${image_type}" --prefix="${cc_prefix}" --destdir="${destdir}"
}

install_cc_sev_image() {
	AA_KBC="offline_sev_kbc"
	image_type="initrd"
	install_cc_image "${AA_KBC}" "${image_type}"
}

#Install CC kernel asset
install_cc_kernel() {
	export kernel_version="$(yq r $versions_yaml assets.kernel.version)"
	DESTDIR="${destdir}" PREFIX="${cc_prefix}" "${kernel_builder}" -f -v "${kernel_version}"
}

# Install static CC qemu asset
install_cc_qemu() {
	info "build static CC qemu"
	export qemu_repo="$(yq r $versions_yaml assets.hypervisor.qemu.url)"
	export qemu_version="$(yq r $versions_yaml assets.hypervisor.qemu.version)"
	"${qemu_cc_builder}"
	tar xvf "${builddir}/kata-static-qemu-cc.tar.gz" -C "${destdir}"
}

#Install all components that are not assets
install_cc_shimv2() {
	GO_VERSION="$(yq r ${versions_yaml} languages.golang.meta.newest-version)"
	export GO_VERSION
	export REMOVE_VMM_CONFIGS="acrn fc"
	DESTDIR="${destdir}" PREFIX="${cc_prefix}" EXTRA_OPTS="DEFSERVICEOFFLOAD=true" "${shimv2_builder}"
}

# Install static CC virtiofsd asset
install_cc_virtiofsd() {
	info "build static CC virtiofsd"
	"${virtiofsd_builder}"
	info "Install static CC virtiofsd"
	mkdir -p "${destdir}/${cc_prefix}/libexec/"
	sudo install -D --owner root --group root --mode 0744 virtiofsd/virtiofsd "${destdir}/${cc_prefix}/libexec/virtiofsd"
}

# Install static CC cloud-hypervisor asset
install_tdx_cc_clh() {
	install_cc_clh
}

#Install CC kernel assert, with TEE support
install_cc_tee_kernel() {
	tee="${1}"
	kernel_version="${2}"

	[[ "${tee}" != "tdx" && "${tee}" != "sev" ]] && die "Non supported TEE"

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
	"${qemu_cc_builder}"
	tar xvf "${builddir}/kata-static-${tee}-qemu-cc.tar.gz" -C "${destdir}"
}

install_cc_tdx_qemu() {
	install_cc_tee_qemu "tdx"
}

install_cc_tdx_td_shim() {
	DESTDIR="${destdir}" PREFIX="${cc_prefix}" "${td_shim_builder}"
	tar xvf "${builddir}/td-shim.tar.gz" -C "${destdir}"
}

install_cc_tee_ovmf() {
	tee="${1}"
	tarball_name="${2}"

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

#Install all components that are not assets
install_shimv2() {
	GO_VERSION="$(yq r ${versions_yaml} languages.golang.meta.newest-version)"
	export GO_VERSION
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
	case "${build_target}" in
	all)
		install_clh
		install_firecracker
		install_image
		install_initrd
		install_kernel
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

	cc-shim-v2) install_cc_shimv2 ;;

	cc-virtiofsd) install_cc_virtiofsd ;;

	cc-tdx-cloud-hypervisor) install_tdx_cc_clh ;;

	cc-tdx-kernel) install_cc_tdx_kernel ;;

	cc-sev-kernel) install_cc_sev_kernel ;;

	cc-tdx-qemu) install_cc_tdx_qemu ;;

	cc-tdx-td-shim) install_cc_tdx_td_shim ;;

	cc-tdx-tdvf) install_cc_tdx_tdvf ;;

	cc-sev-ovmf) install_cc_sev_ovmf ;;

	cloud-hypervisor) install_clh ;;

	firecracker) install_firecracker ;;

	kernel) install_kernel ;;

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

	tarball_name="${workdir}/kata-static-${build_target}.tar.xz"
	(
		cd "${destdir}"
		sudo tar cvfJ "${tarball_name}" "."
	)
	tar tvf "${tarball_name}"
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
