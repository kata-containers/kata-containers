#!/bin/bash
#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e
if [ -n "$DEBUG" ] ; then
	set -x
fi

SCRIPT_NAME="${0##*/}"
IMAGE="${IMAGE:-kata-containers.img}"
AGENT_BIN=${AGENT_BIN:-kata-agent}

die()
{
	local msg="$*"
	echo "ERROR: ${msg}" >&2
	exit 1
}

OK()
{
	local msg="$*"
	echo "[OK] ${msg}" >&2
}

info()
{
	local msg="$*"
	echo "INFO: ${msg}"
}

usage()
{
	error="${1:-0}"
	cat <<EOT
Usage: ${SCRIPT_NAME} [options] <rootfs-dir>
	This script will create a Kata Containers image file based on the 
	<rootfs-dir> directory.

Options:
	-h Show this help
	-o path to generate image file ENV: IMAGE
	-s Image size in MB (default $IMG_SIZE) ENV: IMG_SIZE

Extra environment variables:
	AGENT_BIN: use it to change the expected agent binary name"
EOT
exit "${error}"
}

while getopts "ho:s:" opt
do
	case "$opt" in
		h)	usage ;;
		o)	IMAGE="${OPTARG}" ;;
		s)	IMG_SIZE="${OPTARG}" ;;
	esac
done

shift $(( $OPTIND - 1 ))

ROOTFS="$1"

[ -n "${ROOTFS}" ] || usage
[ -d "${ROOTFS}" ] || die "${ROOTFS} is not a directory"
# The kata rootfs image expect init and kata-agent to be installed
init="${ROOTFS_DIR}/sbin/init"
[ -x "${init}" ] || [ -L ${init} ] || die "/sbin/init is not installed in ${ROOTFS_DIR}"
OK "init is installed"
[ -x "${ROOTFS}/bin/${AGENT_BIN}" ] || \
	die "/bin/${AGENT_BIN} is not installed in ${ROOTFS_DIR}
	use AGENT_BIN env variable to change the expected agent binary name"
OK "Agent installed"
[ "$(id -u)" -eq 0 ] || die "$0: must be run as root"

BLOCK_SIZE=${BLOCK_SIZE:-4096}
IMG_SIZE=${IMG_SIZE:-80}

info "Creating raw disk with size ${IMG_SIZE}M"
qemu-img create -q -f raw "${IMAGE}" "${IMG_SIZE}M"
OK "Image file created"

# Kata runtime expect an image with just one partition
# The partition is the rootfs content

info "Creating partitions"
parted "${IMAGE}" --script "mklabel gpt" \
"mkpart ext4 1M -1M"
OK "Partitions created"

# Get the loop device bound to the image file (requires /dev mounted in the
# image build system and root privileges)
DEVICE=$(losetup -P -f --show "${IMAGE}")

#Refresh partition table
partprobe "${DEVICE}"

MOUNT_DIR=$(mktemp -d osbuilder-mount-dir.XXXX)
info "Formating Image using ext4 format"
mkfs.ext4 -q -F -b "${BLOCK_SIZE}" "${DEVICE}p1"
OK "Image formated"

info "Mounting root paratition"
mount "${DEVICE}p1" "${MOUNT_DIR}"
OK "root paratition mounted"

RESERVED_BLOCKS_PERCENTAGE=3
info "Set filesystem reserved blocks percentage to ${RESERVED_BLOCKS_PERCENTAGE}%"
tune2fs -m "${RESERVED_BLOCKS_PERCENTAGE}" "${DEVICE}p1"

#TODO: Calculate disk size based on rootfs
#FIXME: https://github.com/kata-containers/osbuilder/issues/2
ROOTFS_SIZE=$(du -B 1MB -s "${ROOTFS}" | awk '{print $1}')
AVAIL_DISK=$(df -B M --output=avail "${DEVICE}p1" | tail -1)
AVAIL_DISK=${AVAIL_DISK/M}
info "Free space root partition ${AVAIL_DISK} MB"
info "rootfs size ${ROOTFS_SIZE} MB"
info "Copying content from rootfs to root partition"
cp -a "${ROOTFS}"/* ${MOUNT_DIR}
OK "rootfs copied"

# Cleanup
sync
umount -l ${MOUNT_DIR}
fsck -D -y "${DEVICE}p1"
losetup -d "${DEVICE}"
info "Image created"
