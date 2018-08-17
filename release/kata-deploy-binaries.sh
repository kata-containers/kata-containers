#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly project="kata-containers"
readonly prefix="/opt/kata"
readonly project_to_attach="github.com/${project}/runtime"
readonly tmp_dir=$(mktemp -d -t static-build-tmp.XXXXXXXXXX)
readonly GOPATH="${tmp_dir}/go"
# flag to decide if push tarball to github
push=false
export GOPATH
workdir="${WORKDIR:-$PWD}"

exit_handler() {
	[ -d "${tmp_dir}" ] || sudo rm -rf "${tmp_dir}"
}
trap exit_handler EXIT

projects=(
	proxy
	runtime
	shim
)

die() {
	msg="$*"
	echo "ERROR: ${msg}" >&2
	exit 1
}

info() {
	echo "INFO: $*"
}

usage() {
	return_code=${1:-0}
	cat <<EOT
This script is used as part of the ${project} release process.
It is used to create a tarball with static binaries.


Usage:
${script_name} <options> [version]

Args:
version: The kata version that will be use to create the tarball

options:

-h      : Show this help
-p      : push tarball to ${project_to_attach}
-w <dir>: directory where tarball will be created


EOT

	exit "${return_code}"
}

#Install guest image/initrd asset
install_image() {
	image_destdir="${destdir}/${prefix}/share/kata-containers/"
	info "Create image"
	image_tarball=$(find . -name 'kata-containers-'"${kata_version}"'-*.tar.gz')
	[ -f "${image_tarball}" ] || "${script_dir}/../obs-packaging/kata-containers-image/build_image.sh" -v "${kata_version}"
	image_tarball=$(find . -name 'kata-containers-'"${kata_version}"'-*.tar.gz')
	[ -f "${image_tarball}" ] || die "image not found"
	info "Install image in destdir ${image_tarball}"
	mkdir -p "${image_destdir}"
	tar xf "${image_tarball}" -C "${image_destdir}"
	pushd "${destdir}/${prefix}/share/kata-containers/" >>/dev/null
	info "Create image default symlinks"
	image=$(find . -name 'kata-containers-image*.img')
	initrd=$(find . -name 'kata-containers-initrd*.initrd')
	ln -sf "${image}" kata-containers.img
	ln -sf "${initrd}" kata-containers-initrd.img
	popd >>/dev/null
}

#Install kernel asset
install_kernel() {
	go get "github.com/${project}/packaging" || true
	pushd ${GOPATH}/src/github.com/${project}/packaging >>/dev/null
	git checkout "${kata_version}-kernel-config"
	popd >>/dev/null
	pushd "${script_dir}/../kernel" >>/dev/null

	info "build kernel"
	./build-kernel.sh setup
	./build-kernel.sh build
	info "install kernel"
	DESTDIR="${destdir}" PREFIX="${prefix}" ./build-kernel.sh install
	popd >>/dev/null
}

# Install static qemu asset
install_qemu() {
	info "build static qemu"
	"${script_dir}/../static-build/qemu/build-static-qemu.sh"
	info "Install static qemu"
	tar xf kata-qemu-static.tar.gz -C "${destdir}"
}

#Install all components that are not assets
install_kata_components() {
	for p in "${projects[@]}"; do
		echo "Download ${p}"
		go get "github.com/${project}/$p" || true
		pushd "${GOPATH}/src/github.com/${project}/$p" >>/dev/null
		echo "Checkout to version ${kata_version}"
		git checkout "${kata_version}"
		echo "Build"
		make \
			PREFIX="${prefix}" \
			QEMUCMD="qemu-system-x86_64"
		#TODO Remove libexecdir
		echo "Install"
		make PREFIX="${prefix}" \
			DESTDIR="${destdir}" \
			LIBEXECDIR="/${destdir}/${prefix}/libexec/" \
			install
		popd >>/dev/null
	done
	sed -i -e '/^initrd =/d' "${destdir}/${prefix}/share/defaults/${project}/configuration.toml"
}

main() {
	while getopts "hpw:" opt; do
		case $opt in
		h) usage 0 ;;
		p) push="true" ;;
		w) workdir="${OPTARG}" ;;
		esac
	done
	shift $((OPTIND - 1))

	kata_version=${1:-}
	[ -n "${kata_version}" ] || usage 1
	info "Requested version: ${kata_version}"

	destdir="${workdir}/kata-static-${kata_version}-$(arch)"
	info "DESTDIR ${destdir}"
	mkdir -p "${destdir}"
	install_image
	install_kata_components
	install_kernel
	install_qemu
	tarball_name="${destdir}.tar.xz"
	pushd "${destdir}" >>/dev/null
	tar cfJ "${tarball_name}" "./opt"
	popd >>/dev/null
	if [ "${push}" == "true" ]; then
		hub -C "${GOPATH}/src/github.com/${project}/runtime" release edit -a "${tarball_name}" "${kata_version}"
	else
		echo "Wont push the tarball to github use -p option to do it."
	fi
}

main $@
