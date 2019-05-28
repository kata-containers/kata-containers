#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
source /etc/os-release || source /usr/lib/os-release

KATA_DEV_MODE="${KATA_DEV_MODE:-}"

CURRENT_QEMU_VERSION=$(get_version "assets.hypervisor.qemu.version")
CURRENT_QEMU_TAG=$(get_version "assets.hypervisor.qemu.tag")
QEMU_REPO_URL=$(get_version "assets.hypervisor.qemu.url")
# Remove 'https://' from the repo url to be able to git clone the repo
QEMU_REPO=${QEMU_REPO_URL/https:\/\//}
PACKAGED_QEMU="qemu"
QEMU_ARCH=$(${cidir}/kata-arch.sh -d)

# option "--shallow-submodules" was introduced in git v2.9.0
GIT_SHADOW_VERSION="2.9.0"

get_packaged_qemu_commit() {
	if [ "$ID" == "ubuntu" ] || [ "$ID" == "debian" ]; then
		qemu_commit=$(sudo apt-cache madison $PACKAGED_QEMU \
			| awk '{print $3}' | cut -d'+' -f1 | cut -d':' -f2)
	elif [ "$ID" == "fedora" ]; then
		qemu_commit=$(sudo dnf --showduplicate list ${PACKAGED_QEMU}.${QEMU_ARCH} \
			| awk '/'$PACKAGED_QEMU'/ {print $2}' | cut -d':' -f2 | cut -d'.' -f1-2)
	elif [ "$ID" == "centos" ] || [ "$ID" == "rhel" ]; then
		qemu_commit=$(sudo yum --showduplicate list $PACKAGED_QEMU \
			| awk '/'$PACKAGED_QEMU'/ {print $2}' | cut -d':' -f2 | cut -d'.' -f1-2)
	elif [[ "$ID" =~ ^opensuse.*$ ]] || [ "$ID" == "sles" ]; then
		qemu_commit=$(sudo zypper info $PACKAGED_QEMU \
			| grep "Version" | cut -d':' -f2 | cut -d'.' -f1-2 | tr -d '[:space:]')
	fi

	echo "${qemu_commit}"
}

install_packaged_qemu() {
	rc=0
	# Timeout to download packages from OBS
	limit=180
	if [ "$ID"  == "ubuntu" ] || [ "$ID" == "debian" ]; then
		chronic sudo apt remove -y "$PACKAGED_QEMU" || true
		chronic sudo apt install -y "$PACKAGED_QEMU" || rc=1
	elif [ "$ID"  == "fedora" ]; then
		chronic sudo dnf remove -y "$PACKAGED_QEMU" || true
		chronic sudo dnf install -y "$PACKAGED_QEMU" || rc=1
	elif [ "$ID"  == "centos" ] || [ "$ID"  == "rhel" ]; then
		chronic sudo yum remove -y "$PACKAGED_QEMU" || true
		chronic sudo yum install -y "$PACKAGED_QEMU" || rc=1
	elif [[ "$ID" =~ ^opensuse.*$ ]] || [ "$ID" == "sles" ]; then
		chronic sudo zypper -n remove "$PACKAGED_QEMU" || true
		chronic sudo zypper -n install "$PACKAGED_QEMU" || rc=1
	else
		die "Unrecognized distro"
	fi

	return "$rc"
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
		die "Qemu will not be installed"
	fi

	PACKAGING_REPO="github.com/kata-containers/packaging"
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

	echo "Build Qemu"
	"${QEMU_CONFIG_SCRIPT}" "qemu" | xargs ./configure
	make -j $(nproc)

	echo "Install Qemu"
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
			packaged_qemu_commit=$(get_packaged_qemu_commit)
			short_current_qemu_commit=${CURRENT_QEMU_COMMIT:0:10}
			if [ "$packaged_qemu_commit" == "$short_current_qemu_commit" ]; then
				# If installing packaged qemu from OBS fails,
				# then build and install it from sources.
				install_packaged_qemu || build_and_install_qemu
			else
				build_and_install_qemu
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
