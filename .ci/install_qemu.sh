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

KATA_DEV_MODE="${KATA_DEV_MODE:-}"

CURRENT_QEMU_VERSION=$(get_version "assets.hypervisor.qemu.version")
CURRENT_QEMU_TAG=$(get_version "assets.hypervisor.qemu.tag")
QEMU_REPO_URL=$(get_version "assets.hypervisor.qemu.url")
# Remove 'https://' from the repo url to be able to git clone the repo
QEMU_REPO=${QEMU_REPO_URL/https:\/\//}
QEMU_ARCH=$(${cidir}/kata-arch.sh -d)
PACKAGING_REPO="github.com/kata-containers/packaging"
ARCH=$("${cidir}"/kata-arch.sh -d)
QEMU_TAR="kata-static-qemu.tar.gz"
qemu_latest_build_url="${jenkins_url}/job/qemu-nightly-$(uname -m)/${cached_artifacts_path}"

# option "--shallow-submodules" was introduced in git v2.9.0
GIT_SHADOW_VERSION="2.9.0"

build_static_qemu() {
	info "building static QEMU"
	# only x86_64 is supported for building static QEMU
	[ "$ARCH" != "x86_64" ] && return 1

	go get -d "${PACKAGING_REPO}" || true
	prefix="${KATA_QEMU_DESTDIR}" "${GOPATH}/src/${PACKAGING_REPO}/static-build/qemu/build-static-qemu.sh"

	# We need to move the tar file to a specific location so we
	# can know where it is and then we can perform the build cache
	# operations
	sudo mkdir -p "${KATA_TESTS_CACHEDIR}"
	sudo mv ${QEMU_TAR} ${KATA_TESTS_CACHEDIR}
}

uncompress_static_qemu() {
	local qemu_tar_location="$1"
	[ -n "$qemu_tar_location" ] || die "provide the location of the QEMU compressed file"
	sudo tar -xf "${qemu_tar_location}" -C /
}

build_and_install_static_qemu() {
	build_static_qemu
	uncompress_static_qemu "${KATA_TESTS_CACHEDIR}/${QEMU_TAR}"
}

install_cached_qemu() {
	info "Installing cached QEMU"
	curl -fL --progress-bar "${qemu_latest_build_url}/${QEMU_TAR}" -o "${QEMU_TAR}" || return 1
	curl -fsOL "${qemu_latest_build_url}/sha256sum-${QEMU_TAR}" || return 1

	sha256sum -c "sha256sum-${QEMU_TAR}" || return 1
	uncompress_static_qemu "${QEMU_TAR}"
}

clone_qemu_repo() {
	# check if git is capable of shadow cloning
        git_shadow_clone=$(check_git_version "${GIT_SHADOW_VERSION}")

	if [ "$git_shadow_clone" == "true" ]; then
		git clone --branch "${CURRENT_QEMU_TAG}" --single-branch --depth 1 --shallow-submodules "${QEMU_REPO_URL}" "${GOPATH}/src/${QEMU_REPO}"
	else
		git clone --branch "${CURRENT_QEMU_TAG}" --single-branch --depth 1 "${QEMU_REPO_URL}" "${GOPATH}/src/${QEMU_REPO}"
	fi
}

build_and_install_qemu() {
	if [ -n "$(command -v qemu-system-${QEMU_ARCH})" ] && [ -n "$KATA_DEV_MODE" ]; then
		die "QEMU will not be installed"
	fi

	QEMU_CONFIG_SCRIPT="${GOPATH}/src/${PACKAGING_REPO}/scripts/configure-hypervisor.sh"

	mkdir -p "${GOPATH}/src"
	go get -d "$PACKAGING_REPO" || true

	clone_qemu_repo

	pushd "${GOPATH}/src/${QEMU_REPO}"
	git fetch
	[ -n "$(ls -A capstone)" ] || git clone https://github.com/qemu/capstone.git capstone
	[ -n "$(ls -A ui/keycodemapdb)" ] || git clone  https://github.com/qemu/keycodemapdb.git ui/keycodemapdb

	# Apply required patches
	QEMU_PATCHES_TAG=$(echo "${CURRENT_QEMU_VERSION}" | cut -d '.' -f1-2)
	QEMU_PATCHES_PATH="${GOPATH}/src/${PACKAGING_REPO}/qemu/patches/${QEMU_PATCHES_TAG}.x"
	for patch in ${QEMU_PATCHES_PATH}/*.patch; do
		echo "Applying patch: $patch"
		git apply "$patch"
	done

	echo "Build QEMU"
	"${QEMU_CONFIG_SCRIPT}" "qemu" | xargs ./configure
	make -j $(nproc)

	echo "Install QEMU"
	sudo -E make install
	popd
}

#Load specific configure file
if [ -f "${cidir}/${QEMU_ARCH}/lib_install_qemu_${QEMU_ARCH}.sh" ]; then
	source "${cidir}/${QEMU_ARCH}/lib_install_qemu_${QEMU_ARCH}.sh"
fi

main() {
	case "$QEMU_ARCH" in
		"x86_64")
			cached_qemu_version=$(curl -sfL "${qemu_latest_build_url}/latest") || cached_qemu_version="none"
			info "current QEMU version: $CURRENT_QEMU_VERSION"
			info "cached QEMU version: $cached_qemu_version"

			# When testing initrd, build qemu instead of using the cached qemu
			# as there seems to be an issue with the statically built qemu when
			# running the factory-vm tests.
			if [ "${AGENT_INIT:-}" == "yes" ] || [ -n "${FORCE_BUILD_QEMU:-}" ]; then
				build_and_install_qemu
			elif [ "$cached_qemu_version" == "$CURRENT_QEMU_VERSION" ]; then
				# If installing cached QEMU fails,
				# then build and install it from sources.
				install_cached_qemu || build_and_install_static_qemu
			else
				build_and_install_static_qemu
			fi
			;;
		"ppc64le"|"s390x")
			packaged_qemu_version=$(get_packaged_qemu_version)
			short_current_qemu_version=${CURRENT_QEMU_VERSION#*-}
			if [ "$packaged_qemu_version" == "$short_current_qemu_version" ] && [ -z "${CURRENT_QEMU_TAG}" ] || [ "${QEMU_ARCH}" == "s390x" ]; then
				install_packaged_qemu || build_and_install_qemu
			else
				build_and_install_qemu
			fi
			;;
		"aarch64")
			# For now, we don't follow stable version on aarch64, but one specific tag version, so we need to build from scratch.
			build_and_install_qemu
			;;
		*)
			die "Architecture $QEMU_ARCH not supported"
			;;
	esac
}

main
