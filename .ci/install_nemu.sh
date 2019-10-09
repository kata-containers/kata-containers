#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source "${cidir}/../lib/common.bash"
source /etc/os-release || source /usr/lib/os-release

versions_file="${cidir}/../versions.yaml"
arch=$("${cidir}"/kata-arch.sh -d)
latest_build_url="${jenkins_url}/job/nemu-nightly-${arch}/${cached_artifacts_path}"
nemu_repo=$(get_version "assets.hypervisor.nemu.url")
nemu_version=$(get_version "assets.hypervisor.nemu.version")
NEMU_TAR="kata-nemu-static.tar.gz"

install_nemu() {
	info "build nemu from source"
	PACKAGING_REPO="github.com/kata-containers/packaging"
	case "$arch" in
	x86_64)
		local nemu_bin="nemu-system-${arch}"
		;;
	*)
		die "Unsupported architecture: $arch"
		;;
	esac

	go get -d "${PACKAGING_REPO}" || true

	prefix="${KATA_NEMU_DESTDIR}" ${GOPATH}/src/${PACKAGING_REPO}/static-build/nemu/build-static-nemu.sh "${arch}"
	sudo tar -xvf ${NEMU_TAR} -C /
	# We need to move the tar file to a specific location so we
	# can know where it is and then we can perform the build cache
	# operations
	sudo mkdir -p "${KATA_TESTS_CACHEDIR}"
	sudo mv ${NEMU_TAR} ${KATA_TESTS_CACHEDIR}
}

install_firmware() {
	local firmware="OVMF.fd"
	local firmware_repo=$(get_version "assets.hypervisor.nemu-ovmf.url")
	local firmware_version=$(get_version "assets.hypervisor.nemu-ovmf.version")
	local firmware_dir="/usr/share/nemu/firmware"

	sudo mkdir -p "${firmware_dir}"
	curl -LO "${firmware_repo}/releases/download/${firmware_version}/${firmware}"
	sudo install -o root -g root -m 0644 "${firmware}" "${firmware_dir}"
	rm -f "${firmware}"
}

install_prebuilt_nemu() {
	sudo -E curl -fL --progress-bar "${latest_build_url}/${NEMU_TAR}" -o "${NEMU_TAR}" || return 1
	info "Install pre-built nemu version"
	sudo tar -xvf "${NEMU_TAR}" -C /

	info "Verify download checksum"
	sudo -E curl -fsOL "${latest_build_url}/sha256sum-${NEMU_TAR}" || return 1
	sudo sha256sum -c "sha256sum-${NEMU_TAR}" || return 1
}

main() {
	cached_nemu_version=$(curl -sfL "${latest_build_url}/latest") || cached_nemu_version="none"
	info "current nemu : ${nemu_version}"
	info "cached nemu  : ${cached_nemu_version}"
	# Currently in our CI we are only installing and running nemu on x86_64 that is the
	# main reason of why we will only install the prebuilt nemu on this arch, this
	# can change once that we have nemu installation on other archs
	if [ "$cached_nemu_version" == "$nemu_version" ] && [ "$arch" == "x86_64" ]; then
		# If installing nemu fails,
		# then build and install it from sources.
		if ! install_prebuilt_nemu; then
			info "failed to install cached nemu, trying to build from source"
			install_nemu
		fi
	else
		install_nemu
	fi
}

main
