#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")

source /etc/os-release
source "${cidir}/lib.sh"

AGENT_INIT=${AGENT_INIT:-no}
TEST_INITRD=${TEST_INITRD:-no}

TMP_DIR=
ROOTFS_DIR=

PACKAGED_IMAGE="kata-containers-image"
IMG_PATH="/usr/share/kata-containers"
IMG_NAME="kata-containers.img"

agent_path="${GOPATH}/src/github.com/kata-containers/agent"

IMG_MOUNT_DIR=
LOOP_DEVICE=

# Build Kata agent
bash -f "${cidir}/install_agent.sh"
agent_commit=$(git --work-tree="${agent_path}" --git-dir="${agent_path}/.git" log --format=%h -1 HEAD)

cleanup() {
	[ -d "${ROOTFS_DIR}" ] && [[ "${ROOTFS_DIR}" = *"rootfs"* ]] && sudo rm -rf "${ROOTFS_DIR}"
	[ -d "${TMP_DIR}" ] && rm -rf "${TMP_DIR}"
	if [ -n "${IMG_MOUNT_DIR}" ] && mount | grep -q "${IMG_MOUNT_DIR}"; then
		sudo umount "${IMG_MOUNT_DIR}"
	fi
	if [ -d "${IMG_MOUNT_DIR}" ]; then
		rm -rf "${IMG_MOUNT_DIR}"
	fi
	if [ -n "${LOOP_DEVICE}" ]; then
		sudo losetup -d "${LOOP_DEVICE}"
	fi
}

trap cleanup EXIT

get_packaged_agent_version() {
	version=$(ls "$IMG_PATH" | grep "$PACKAGED_IMAGE" | cut -d'_' -f4 | cut -d'.' -f1)
	if [ -z "$version" ]; then
		die "unknown agent version"
	fi
	echo "$version"
}

install_packaged_image() {
	if [ "$ID"  == "ubuntu" ]; then
		sudo -E apt install -y "$PACKAGED_IMAGE"
	elif [ "$ID"  == "fedora" ]; then
		sudo -E dnf install -y "$PACKAGED_IMAGE"
	elif [ "$ID"  == "centos" ]; then
		sudo -E yum install -y "$PACKAGED_IMAGE"
	else
		die "Linux distribution not supported"
	fi
}

update_agent() {
	pushd "$agent_path"

	LOOP_DEVICE=$(sudo losetup -f --show "${IMG_PATH}/${IMG_NAME}")
	IMG_MOUNT_DIR=$(mktemp -d -t kata-image-mount.XXXXXXXXXX)
	sudo partprobe "$LOOP_DEVICE"
	sudo mount "${LOOP_DEVICE}p1" "$IMG_MOUNT_DIR"

	echo "Old agent version:"
	"${IMG_MOUNT_DIR}/usr/bin/kata-agent" --version

	echo "Install new agent"
	sudo -E PATH="$PATH" bash -c "make install DESTDIR=$IMG_MOUNT_DIR"
	installed_version=$("${IMG_MOUNT_DIR}/usr/bin/kata-agent" --version)
	echo "New agent version: $installed_version"

	popd
	installed_version=${installed_version##k*-}
	[[ "${installed_version}" == *"${current_version}"* ]]
}

build_image() {
	TMP_DIR=$(mktemp -d -t kata-image-install.XXXXXXXXXX)
	readonly ROOTFS_DIR="${TMP_DIR}/rootfs"
	export ROOTFS_DIR

	image_type=$(get_version "assets.image.meta.image-type")
	OSBUILDER_DISTRO=${OSBUILDER_DISTRO:-$image_type}

	osbuilder_repo="github.com/kata-containers/osbuilder"

	# Clone os-builder repository
	go get -d "${osbuilder_repo}" || true

	(cd "${GOPATH}/src/${osbuilder_repo}/rootfs-builder" && \
		sudo -E AGENT_INIT="${AGENT_INIT}" AGENT_VERSION="${agent_commit}" \
		GOPATH="$GOPATH" USE_DOCKER=true ./rootfs.sh "${OSBUILDER_DISTRO}")

	# Build the image
	if [ x"${TEST_INITRD}" == x"yes" ]; then
		pushd "${GOPATH}/src/${osbuilder_repo}/initrd-builder"
		sudo -E AGENT_INIT="${AGENT_INIT}" USE_DOCKER=true ./initrd_builder.sh "$ROOTFS_DIR"
		image_name="kata-containers-initrd.img"
	else
		pushd "${GOPATH}/src/${osbuilder_repo}/image-builder"
		sudo -E AGENT_INIT="${AGENT_INIT}" USE_DOCKER=true ./image_builder.sh "$ROOTFS_DIR"
		image_name="kata-containers.img"
	fi

	# Install the image
	commit=$(git log --format=%h -1 HEAD)
	date=$(date +%Y-%m-%d-%T.%N%z)
	image="kata-containers-${date}-osbuilder-${commit}-agent-${agent_commit}"

	sudo install -o root -g root -m 0640 -D ${image_name} "/usr/share/kata-containers/${image}"
	(cd /usr/share/kata-containers && sudo ln -sf "$image" ${image_name})

	popd
}

main() {
	if [ x"${TEST_INITRD}" == x"yes" ]; then
		build_image
	else
		install_packaged_image
		packaged_version=$(get_packaged_agent_version)
		current_version=${agent_commit}
		if [ "$packaged_version" != "$current_version" ]; then
				update_agent || build_image
		fi
	fi
}

main
