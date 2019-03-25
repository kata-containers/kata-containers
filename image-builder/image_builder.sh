#!/usr/bin/env bash
#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

[ -n "$DEBUG" ] && set -x

script_name="${0##*/}"
script_dir="$(dirname $(readlink -f $0))"

lib_file="${script_dir}/../scripts/lib.sh"
source "$lib_file"

[ "$(id -u)" -eq 0 ] || die "$0: must be run as root"

IMAGE="${IMAGE:-kata-containers.img}"
IMG_SIZE=128
AGENT_BIN=${AGENT_BIN:-kata-agent}
AGENT_INIT=${AGENT_INIT:-no}
DAX=${DAX:-no}
DAX_HEADER_SZ=2


usage()
{
	error="${1:-0}"
	cat <<EOT
Usage: ${script_name} [options] <rootfs-dir>
	This script will create a Kata Containers image file of
	an adequate size based on the <rootfs-dir> directory.

Options:
	-h Show this help
	-o path to generate image file ENV: IMAGE
	-r Free space of the root partition in MB ENV: ROOT_FREE_SPACE

Extra environment variables:
	AGENT_BIN:  use it to change the expected agent binary name
	AGENT_INIT: use kata agent as init process
	DAX: If 'yes' will build the image with DAX support. The first 2 MB of the
	     resulting image are reserved for the device namespace information
	     (metadata) that is used by the guest kernel to enable DAX.
	USE_DOCKER: If set will build image in a Docker Container (requries docker)
	            DEFAULT: not set


	When DAX is 'yes', the following diagram shows how a 128M image will looks like:
		 .-----------------------------------.
		 |-- 2 MB --|-------- 126 MB --------|
		 | Metadata | Rootfs (/bin,/usr,etc) |
		 '-----------------------------------'

		 The resulting image can be mounted if the offset of 2 MB is specified:
		 $ sudo losetup -v -fP -o $((2*1024*1024)) kata-containers.img

EOT
exit "${error}"
}

# Maximum allowed size in MB for root disk
MAX_IMG_SIZE_MB=2048

FS_TYPE=${FS_TYPE:-"ext4"}

# In order to support memory hotplug, image must be aligned to memory section(size in MB) according to different architecture.
ARCH=$(uname -m)
case "$ARCH" in
	aarch64)	MEM_BOUNDARY_MB=1024 ;;
	      *)        MEM_BOUNDARY_MB=128  ;;
esac

# Maximum no of attempts to create a root disk before giving up
MAX_ATTEMPTS=5

ATTEMPT_NUM=0
while getopts "ho:r:s:f:" opt
do
	case "$opt" in
		h)	usage ;;
		o)	IMAGE="${OPTARG}" ;;
		r)	ROOT_FREE_SPACE="${OPTARG}" ;;
		f)	FS_TYPE="${OPTARG}" ;;
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
		--rm \
		--runtime runc  \
		--privileged \
		--env IMG_SIZE="${IMG_SIZE}" \
		--env AGENT_INIT=${AGENT_INIT} \
		--env DAX="${DAX}" \
		--env DEBUG="${DEBUG}" \
		-v /dev:/dev \
		-v "${script_dir}":"/osbuilder" \
		-v "${script_dir}/../scripts":"/scripts" \
		-v "${ROOTFS}":"/rootfs" \
		-v "${IMAGE_DIR}":"/image" \
		${image_name} \
		bash "/osbuilder/${script_name}" -o "/image/${IMAGE_NAME}" /rootfs

	exit $?
fi
# The kata rootfs image expect init and kata-agent to be installed
init_path="/sbin/init"
init="${ROOTFS}${init_path}"
[ -x "${init}" ] || [ -L ${init} ] || die "${init_path} is not installed in ${ROOTFS}"
OK "init is installed"

if [ "${AGENT_INIT}" == "no" ]
then
	systemd_path="/lib/systemd/systemd"
	systemd="${ROOTFS}${systemd_path}"
	[ -x "${systemd}" ] || [ -L ${systemd} ] || die "${systemd_path} is not installed in ${ROOTFS}"
	OK "init is systemd"
fi

[ "${AGENT_INIT}" == "yes" ] || [ -x "${ROOTFS}/usr/bin/${AGENT_BIN}" ] || \
	die "/usr/bin/${AGENT_BIN} is not installed in ${ROOTFS}
	use AGENT_BIN env variable to change the expected agent binary name"
OK "Agent installed"

ROOTFS_SIZE=$(du -B 1MB -s "${ROOTFS}" | awk '{print $1}')
BLOCK_SIZE=${BLOCK_SIZE:-4096}
OLD_IMG_SIZE=0
ORIG_MEM_BOUNDARY_MB=${MEM_BOUNDARY_MB}

align_memory()
{
	remaining=$(($IMG_SIZE % $MEM_BOUNDARY_MB))
	if [ "$remaining" != "0" ];then
		warning "image size '$IMG_SIZE' is not aligned to memory boundary '$MEM_BOUNDARY_MB', aligning it"
		IMG_SIZE=$(($IMG_SIZE + $MEM_BOUNDARY_MB - $remaining))
	fi


	if [ "${DAX}" == "yes" ] ; then
		# To support:
		# * memory hotplug: the image size MUST BE aligned to MEM_BOUNDARY_MB (128 or 1024 MB)
		# * DAX: NVDIMM driver reads the device namespace information from nvdimm namespace (4K offset).
		#        The namespace information is saved in the first 2MB of the image.
		# * DAX huge pages [2]: 2MB alignment
		#
		# [1] - nd_pfn_validate(): https://github.com/torvalds/linux/blob/master/drivers/nvdimm/pfn_devs.c
		# [2] - https://nvdimm.wiki.kernel.org/2mib_fs_dax
		IMG_SIZE=$((IMG_SIZE-DAX_HEADER_SZ))
	fi
}

# Calculate image size based on the rootfs
calculate_img_size()
{
	IMG_SIZE=${IMG_SIZE:-$MEM_BOUNDARY_MB}
	align_memory
	if [ -n "$ROOT_FREE_SPACE" ] && [ "$IMG_SIZE" -gt "$ROOTFS_SIZE" ]; then
		info "Ensure that root partition has at least ${ROOT_FREE_SPACE}MB of free space"
		IMG_SIZE=$(($IMG_SIZE + $ROOT_FREE_SPACE))
	fi

}

unmount()
{
	sync
	umount -l ${MOUNT_DIR}
	rmdir ${MOUNT_DIR}
}

detach()
{
	losetup -d "${DEVICE}"

	# From `man losetup` about -d option:
	# Note that since Linux v3.7 kernel uses "lazy device destruction".
	# The detach operation does not return EBUSY  error  anymore  if
	# device is actively used by system, but it is marked by autoclear
	# flag and destroyed later
	info "Waiting for ${DEVICE} to detach"

	local i=0
	local max_tries=5
	while [[ "$i" < "$max_tries" ]]; do
		sleep 1
		# If either the 'p1' partition has disappeared or partprobe failed, then
		# the loop device should be correctly detached
		if ! [ -b "${DEVICE}p1" ] || ! partprobe -s ${DEVICE}; then
			break
		fi
		((i+=1))
		echo -n "."
	done

	[[ "$i" == "$max_tries" ]] && die "Cannot detach ${DEVICE}"
	info "detached"
}


create_rootfs_disk()
{
	ATTEMPT_NUM=$(($ATTEMPT_NUM+1))
	if [ ${ATTEMPT_NUM} -gt ${MAX_ATTEMPTS} ]; then
		die "Unable to create root disk image."
	fi
	info "Create root disk image. Attempt ${ATTEMPT_NUM} out of ${MAX_ATTEMPTS}."

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
	parted -s -a optimal "${IMAGE}" \
		   mklabel gpt -- \
		   mkpart primary "${FS_TYPE}" 1M -1M \
		   print
	OK "Partitions created"

	# Get the loop device bound to the image file (requires /dev mounted in the
	# image build system and root privileges)
	DEVICE=$(losetup -P -f --show "${IMAGE}")

	#Refresh partition table
	partprobe -s "${DEVICE}"
	# Poll for the block device p1
	local i=0
	local max_tries=5
	while [[ "$i" < "$max_tries" ]]; do
		[ -b "${DEVICE}p1" ] && break
		((i+=1))
		echo -n "."
		sleep 1
	done
	[[ "$i" == "$max_tries" ]] && die "File ${DEVICE}p1 is not a block device"

	MOUNT_DIR=$(mktemp -d osbuilder-mount-dir.XXXX)
	info "Formatting Image using ext4 filesystem"
	mkfs.ext4 -q -F -b "${BLOCK_SIZE}" "${DEVICE}p1"
	OK "Image formatted"

	info "Mounting root partition"
	mount "${DEVICE}p1" "${MOUNT_DIR}"
	OK "root partition mounted"
	RESERVED_BLOCKS_PERCENTAGE=3
	info "Set filesystem reserved blocks percentage to ${RESERVED_BLOCKS_PERCENTAGE}%"
	tune2fs -m "${RESERVED_BLOCKS_PERCENTAGE}" "${DEVICE}p1"

	AVAIL_DISK=$(df -B M --output=avail "${DEVICE}p1" | tail -1)
	AVAIL_DISK=${AVAIL_DISK/M}
	info "Free space root partition ${AVAIL_DISK} MB"

	# if the available disk space is less than rootfs size, repeat the process
	# of disk creation by adding 5% in the inital assumed value $ROOTFS_SIZE
	if [ $ROOTFS_SIZE -gt $AVAIL_DISK ]; then
		# Increase the size but remain aligned to the original MEM_BOUNDARY_MB, which is stored in $ORIG_MEM_BOUNDARY_MB
		MEM_BOUNDARY_MB=$((MEM_BOUNDARY_MB+ORIG_MEM_BOUNDARY_MB))
		OLD_IMG_SIZE=${IMG_SIZE}
		unset IMG_SIZE
		unmount
		detach
		rm -f ${IMAGE}
		create_rootfs_disk
	fi
}

set_dax_metadata()
{
	dax_header_bytes=$((DAX_HEADER_SZ*1024*1024))
	info "Set device namespace information (metadata)"
	# Fill out namespace information
	tmp_img="$(mktemp)"
	chmod 0644 "${tmp_img}"
	# metadate header
	dd if=/dev/zero of="${tmp_img}" bs="${DAX_HEADER_SZ}M" count=1
	# append image data (rootfs)
	dd if="${IMAGE}" of="${tmp_img}" oflag=append conv=notrunc
	# copy final image
	mv "${tmp_img}" "${IMAGE}"
	# Set metadata header
	# Issue: https://github.com/kata-containers/osbuilder/issues/240
	gcc -O2 "${script_dir}/nsdax.gpl.c" -o "${script_dir}/nsdax"
	"${script_dir}/nsdax" "${IMAGE}" "${dax_header_bytes}" "${dax_header_bytes}"
	sync
}

create_rootfs_disk

info "rootfs size ${ROOTFS_SIZE} MB"
info "Copying content from rootfs to root partition"
cp -a "${ROOTFS}"/* ${MOUNT_DIR}
sync
OK "rootfs copied"

unmount
# Optimize
fsck.ext4 -D -y "${DEVICE}p1"
detach

if [ "${DAX}" == "yes" ] ; then
	set_dax_metadata
fi

info "Image created. Virtual size: ${IMG_SIZE}MB."
