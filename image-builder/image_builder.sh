#!/bin/bash
#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

script_name="${0##*/}"
script_dir="$(dirname $(readlink -f $0))"

if [ -n "$DEBUG" ] ; then
	set -x
fi

SCRIPT_NAME="${0##*/}"
IMAGE="${IMAGE:-kata-containers.img}"
AGENT_BIN=${AGENT_BIN:-kata-agent}
AGENT_INIT=${AGENT_INIT:-no}

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

warning()
{
        local msg="$*"
        echo "WARNING: ${msg}"
}

usage()
{
	error="${1:-0}"
	cat <<EOT
Usage: ${SCRIPT_NAME} [options] <rootfs-dir>
	This script will create a Kata Containers image file of
        an adequate size based on the <rootfs-dir> directory.
        The size of the image can be also be specified manually
        by '-s' flag.

Options:
	-h Show this help
	-o path to generate image file ENV: IMAGE
	-s Image size in MB ENV: IMG_SIZE

Extra environment variables:
	AGENT_BIN:  use it to change the expected agent binary name
	AGENT_INIT: use kata agent as init process
	USE_DOCKER: If set will build image in a Docker Container (requries docker)
	            DEFAULT: not set
EOT
exit "${error}"
}

# Maximum allowed size in MB for root disk
MAX_IMG_SIZE_MB=2048

FS_TYPE=${FS_TYPE:-"ext4"}

# In order to support memory hotplug, image must be aligned to 128M
MEM_BOUNDARY=128

# Maximum no of attempts to create a root disk before giving up
MAX_ATTEMPTS=5

ATTEMPT_NUM=0
while getopts "ho:s:f:" opt
do
	case "$opt" in
		h)	usage ;;
		o)	IMAGE="${OPTARG}" ;;
		s)	IMG_SIZE=${OPTARG}
                        if [ ${IMG_SIZE} -lt 0 ]; then
                                die "Image size has to be greater than 0 MB."
                        fi
                        if [ ${IMG_SIZE} -gt ${MAX_IMG_SIZE_MB} ]; then
                                die "Image size should not be greater than ${MAX_IMG_SIZE_MB} MB."
                        fi
                        ;;
                f)      FS_TYPE="${OPTARG}" ;;
	esac
done

shift $(( $OPTIND - 1 ))

ROOTFS="$1"


[ -n "${ROOTFS}" ] || usage
[ -d "${ROOTFS}" ] || die "${ROOTFS} is not a directory"

ROOTFS=$(readlink -f ${ROOTFS})
IMAGE_DIR=$(dirname ${IMAGE})
IMAGE_DIR=$(readlink -f ${IMAGE_DIR})
IMAGE_NAME=$(basename ${IMAGE})

if [ -n "${USE_DOCKER}" ] ; then
	image_name="image-builder-osbuilder"

	docker build  \
		--build-arg http_proxy="${http_proxy}" \
		--build-arg https_proxy="${https_proxy}" \
		-t "${image_name}" "${script_dir}"

	#Make sure we use a compatible runtime to build rootfs
	# In case Clear Containers Runtime is installed we dont want to hit issue:
	#https://github.com/clearcontainers/runtime/issues/828
	docker run  \
		--runtime runc  \
		--privileged \
		--env IMG_SIZE="${IMG_SIZE}" \
		--env AGENT_INIT=${AGENT_INIT} \
		-v /dev:/dev \
		-v "${script_dir}":"/osbuilder" \
		-v "${ROOTFS}":"/rootfs" \
		-v "${IMAGE_DIR}":"/image" \
		${image_name} \
		bash "/osbuilder/${script_name}" -o "/image/${IMAGE_NAME}" /rootfs

	exit $?
fi
# The kata rootfs image expect init and kata-agent to be installed
init="${ROOTFS}/sbin/init"
[ -x "${init}" ] || [ -L ${init} ] || die "/sbin/init is not installed in ${ROOTFS_DIR}"
OK "init is installed"
[ "${AGENT_INIT}" == "yes" ] || [ -x "${ROOTFS}/bin/${AGENT_BIN}" ] || \
	die "/bin/${AGENT_BIN} is not installed in ${ROOTFS}
	use AGENT_BIN env variable to change the expected agent binary name"
OK "Agent installed"
[ "$(id -u)" -eq 0 ] || die "$0: must be run as root"

ROOTFS_SIZE=$(du -B 1MB -s "${ROOTFS}" | awk '{print $1}')
BLOCK_SIZE=${BLOCK_SIZE:-4096}
OLD_IMG_SIZE=0

align_memory()
{
    remaining=$(($IMG_SIZE % $MEM_BOUNDARY))
    if [ "$remaining" != "0" ];then
        warning "image size '$IMG_SIZE' is not aligned to memory boundary '$MEM_BOUNDARY', aligning it"
        IMG_SIZE=$(($IMG_SIZE + $MEM_BOUNDARY - $remaining))
    fi
}

# Calculate image size based on the rootfs
calculate_img_size()
{
    IMG_SIZE=${IMG_SIZE:-$MEM_BOUNDARY}
    align_memory
}

# Cleanup
cleanup()
{
    sync
    umount -l ${MOUNT_DIR}
    rmdir ${MOUNT_DIR}
    fsck -D -y "${DEVICE}p1"
    losetup -d "${DEVICE}"
}

create_rootfs_disk()
{
    ATTEMPT_NUM=$(($ATTEMPT_NUM+1))
    info "Create root disk image. Attempt ${ATTEMPT_NUM} out of ${MAX_ATTEMPTS}."
    if [ ${ATTEMPT_NUM} -gt ${MAX_ATTEMPTS} ]; then
        die "Unable to create root disk image."
    fi

    calculate_img_size
    if [ ${OLD_IMG_SIZE} -ne 0 ]; then
        info "Image size ${OLD_IMG_SIZE}MB too small, trying again with size ${IMG_SIZE}MB"
    fi

    info "Creating raw disk with size ${IMG_SIZE}M"
    qemu-img create -q -f raw "${IMAGE}" "${IMG_SIZE}M"
    OK "Image file created"

    # Kata runtime expect an image with just one partition
    # The partition is the rootfs content

    info "Creating partitions"
    parted "${IMAGE}" --script "mklabel gpt" \
    "mkpart ${FS_TYPE} 1M -1M"
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

    AVAIL_DISK=$(df -B M --output=avail "${DEVICE}p1" | tail -1)
    AVAIL_DISK=${AVAIL_DISK/M}
    info "Free space root partition ${AVAIL_DISK} MB"

    # if the available disk space is less than rootfs size, repeat the process
    # of disk creation by adding 5% in the inital assumed value $ROOTFS_SIZE
    if [ $ROOTFS_SIZE -gt $AVAIL_DISK ]; then
        # Increase the size but remain aligned to 128
        MEM_BOUNDARY=$(($MEM_BOUNDARY+128))
        rm -f ${IMAGE}
        OLD_IMG_SIZE=${IMG_SIZE}
        unset IMG_SIZE
        cleanup
        create_rootfs_disk
    fi
}

create_rootfs_disk

info "rootfs size ${ROOTFS_SIZE} MB"
info "Copying content from rootfs to root partition"
cp -a "${ROOTFS}"/* ${MOUNT_DIR}
OK "rootfs copied"

cleanup

info "Image created. Size: ${IMG_SIZE}MB."
