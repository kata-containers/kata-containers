#!/usr/bin/env bash
#
# Copyright (c) 2017-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

[ -z "${DEBUG}" ] || set -x

set -o errexit
# set -o nounset
set -o pipefail

DOCKER_RUNTIME=${DOCKER_RUNTIME:-runc}
MEASURED_ROOTFS=${MEASURED_ROOTFS:-no}

#For cross build
CROSS_BUILD=${CROSS_BUILD:-false}
BUILDX=""
PLATFORM=""
TARGET_ARCH=${TARGET_ARCH:-$(uname -m)}
ARCH=${ARCH:-$(uname -m)}
[ "${TARGET_ARCH}" == "aarch64" ] && TARGET_ARCH=arm64
TARGET_OS=${TARGET_OS:-linux}
[ "${CROSS_BUILD}" == "true" ] && BUILDX=buildx && PLATFORM="--platform=${TARGET_OS}/${TARGET_ARCH}"

readonly script_name="${0##*/}"
readonly script_dir=$(dirname "$(readlink -f "$0")")
readonly lib_file="${script_dir}/../scripts/lib.sh"

readonly ext4_format="ext4"
readonly xfs_format="xfs"

# ext4: percentage of the filesystem which may only be allocated by privileged processes.
readonly reserved_blocks_percentage=3

# Where the rootfs starts in MB
readonly rootfs_start=1

# Where the rootfs ends in MB
readonly rootfs_end=-1

# DAX header size
# * NVDIMM driver reads the device namespace information from nvdimm namespace (4K offset).
#   The MBR #1 + DAX metadata are saved in the first 2MB of the image.
readonly dax_header_sz=2

# DAX aligment
# * DAX huge pages [2]: 2MB alignment
# [2] - https://nvdimm.wiki.kernel.org/2mib_fs_dax
readonly dax_alignment=2

# Set a default value
AGENT_INIT=${AGENT_INIT:-no}
SELINUX=${SELINUX:-no}
SELINUXFS="/sys/fs/selinux"

# Align image to 128M
readonly mem_boundary_mb=128

# shellcheck source=../scripts/lib.sh
source "${lib_file}"

usage() {
	cat <<EOF
Usage: ${script_name} [options] <rootfs-dir>
	This script will create a Kata Containers image file of
	an adequate size based on the <rootfs-dir> directory.

Options:
	-h Show this help
	-o Path to generate image file. ENV: IMAGE
	-r Free space of the root partition in MB. ENV: ROOT_FREE_SPACE
	-f Filesystem type to use, only ext4, xfs and erofs are supported. ENV: FS_TYPE

Extra environment variables:
	AGENT_BIN:      Use it to change the expected agent binary name
	AGENT_INIT:     Use kata agent as init process
	BLOCK_SIZE:     Use to specify the size of blocks in bytes. DEFAULT: 4096
	IMAGE_REGISTRY: Hostname for the image registry used to pull down the rootfs build image.
	NSDAX_BIN:      Use to specify path to pre-compiled 'nsdax' tool.
	USE_DOCKER:     If set will build image in a Docker Container (requries docker)
	                DEFAULT: not set
	USE_PODMAN:     If set and USE_DOCKER not set, will build image in a Podman Container (requries podman)
	                DEFAULT: not set
	SELINUX:        If set to "yes", the rootfs is labeled for SELinux.
	                Make sure that selinuxfs is mounted to /sys/fs/selinux on the host
	                and the rootfs is built with SELINUX=yes.
	                DEFAULT value: "no"

Following diagram shows how the resulting image will look like

	.-----------.----------.---------------.-----------.
	| 0 - 512 B | 4 - 8 Kb |  2M - 2M+512B |    3M     |
	|-----------+----------+---------------+-----------+
	|   MBR #1  |   DAX    |    MBR #2     |  Rootfs   |
	'-----------'----------'---------------'-----------+
	      |          |      ^      |        ^
	      |          '-data-'      '--------'
	      |                                 |
	      '--------rootfs-partition---------'


MBR: Master boot record.
DAX: Metadata required by the NVDIMM driver to enable DAX in the guest [1][2] (struct nd_pfn_sb).
Rootfs: partition that contains the root filesystem (/usr, /bin, ect).

Kernels and hypervisors that support DAX/NVDIMM read the MBR #2, otherwise MBR #1 is read.

[1] - https://github.com/kata-containers/kata-containers/blob/main/tools/osbuilder/image-builder/nsdax.gpl.c
[2] - https://github.com/torvalds/linux/blob/master/drivers/nvdimm/pfn.h

EOF
}


# build the image using container engine
build_with_container() {
	local rootfs="$1"
	local image="$2"
	local fs_type="$3"
	local block_size="$4"
	local root_free_space="$5"
	local agent_bin="$6"
	local agent_init="$7"
	local container_engine="$8"
	local nsdax_bin="$9"
	local container_image_name="image-builder-osbuilder"
	local shared_files=""
	local selinuxfs=""

	image_dir=$(readlink -f "$(dirname "${image}")")
	image_name=$(basename "${image}")

	engine_build_args=""
	if [ -n "${IMAGE_REGISTRY}" ]; then
		engine_build_args+=" --build-arg IMAGE_REGISTRY=${IMAGE_REGISTRY}"
	fi
	if [ -n "${USE_PODMAN}" ]; then
		engine_build_args+=" --runtime ${DOCKER_RUNTIME}"
	fi

	"${container_engine}" ${BUILDX} build ${PLATFORM}  \
		   ${engine_build_args} \
		   --build-arg http_proxy="${http_proxy}" \
		   --build-arg https_proxy="${https_proxy}" \
		   -t "${container_image_name}" "${script_dir}"

	readonly mke2fs_conf="/etc/mke2fs.conf"
	if [ -f "${mke2fs_conf}" ]; then
		shared_files+="-v ${mke2fs_conf}:${mke2fs_conf}:ro "
	fi

	if [ "${SELINUX}" == "yes" ]; then
		if mountpoint $SELINUXFS > /dev/null; then
			selinuxfs="-v ${SELINUXFS}:${SELINUXFS}"
		else
			die "Make sure that SELinux is enabled on the host"
		fi
	fi

	#Make sure we use a compatible runtime to build rootfs
	# In case Clear Containers Runtime is installed we dont want to hit issue:
	#https://github.com/clearcontainers/runtime/issues/828
	"${container_engine}" run  \
		   --rm \
		   --runtime "${DOCKER_RUNTIME}"  \
		   --privileged \
		   --env AGENT_BIN="${agent_bin}" \
		   --env AGENT_INIT="${agent_init}" \
		   --env FS_TYPE="${fs_type}" \
		   --env BLOCK_SIZE="${block_size}" \
		   --env ROOT_FREE_SPACE="${root_free_space}" \
		   --env NSDAX_BIN="${nsdax_bin}" \
		   --env MEASURED_ROOTFS="${MEASURED_ROOTFS}" \
		   --env SELINUX="${SELINUX}" \
		   --env DEBUG="${DEBUG}" \
		   --env ARCH="${ARCH}" \
		   --env TARGET_ARCH="${TARGET_ARCH}" \
		   --env USER="$(id -u)" \
		   --env GROUP="$(id -g)" \
		   -v /dev:/dev \
		   -v "${script_dir}":"/osbuilder" \
		   -v "${script_dir}/../scripts":"/scripts" \
		   -v "${rootfs}":"/rootfs" \
		   -v "${image_dir}":"/image" \
		   ${selinuxfs} \
		   ${shared_files} \
		   ${container_image_name} \
		   bash "/osbuilder/${script_name}" -o "/image/${image_name}" /rootfs
}

check_rootfs() {
	local rootfs="${1}"

	[ -d "${rootfs}" ] || die "${rootfs} is not a directory"

	# The kata rootfs image expect init and kata-agent to be installed
	init_path="/sbin/init"
	init="${rootfs}${init_path}"
	if [ ! -x "${init}" ] && [ ! -L "${init}" ]; then
		error "${init_path} is not installed in ${rootfs}"
		return 1
	fi
	OK "init is installed"


	candidate_systemd_paths="/usr/lib/systemd/systemd /lib/systemd/systemd"

	# check agent or systemd
	case "${AGENT_INIT}" in
		"no")
			for systemd_path in $candidate_systemd_paths; do
				systemd="${rootfs}${systemd_path}"
				if [ -x "${systemd}" ] || [ -L "${systemd}" ]; then
					found="yes"
					break
				fi
			done
			if [ ! $found ]; then
				error "None of ${candidate_systemd_paths} is installed in ${rootfs}"
				return 1
			fi
			OK "init is systemd"
			;;

		"yes")
			agent_path="/sbin/init"
			agent="${rootfs}${agent_path}"
			if  [ ! -x "${agent}" ]; then
				error "${agent_path} is not installed in ${rootfs}. Use AGENT_BIN env variable to change the expected agent binary name"
				return 1
			fi
			# checksum must be different to system
			for systemd_path in $candidate_systemd_paths; do
				systemd="${rootfs}${systemd_path}"
				if [ -f "${systemd}" ] && cmp -s "${systemd}" "${agent}"; then
					error "The agent is not the init process. ${agent_path} is systemd"
					return 1
				fi
			done

			OK "Agent installed"
			;;

		*)
			error "Invalid value for AGENT_INIT: '${AGENT_INIT}'. Use to 'yes' or 'no'"
			return 1
			;;
	esac

	return 0
}

calculate_required_disk_size() {
	local rootfs="$1"
	local fs_type="$2"
	local block_size="$3"

	readonly rootfs_size_mb=$(du -B 1M -s "${rootfs}" | awk '{print $1}')
	readonly image="$(mktemp)"
	readonly mount_dir="$(mktemp -d)"
	readonly max_tries=20
	readonly increment=10

	for i in $(seq 1 $max_tries); do
		local img_size="$((rootfs_size_mb + (i * increment)))"
		create_disk "${image}" "${img_size}" "${fs_type}" "${rootfs_start}" > /dev/null 2>&1
		if ! device="$(setup_loop_device "${image}")"; then
			continue
		fi

		if ! format_loop "${device}" "${block_size}" "${fs_type}" > /dev/null 2>&1 ; then
			die "Could not format loop device: ${device}"
		fi
		mount "${device}p1" "${mount_dir}"
		avail="$(df -BM --output=avail "${mount_dir}" | tail -n1 | sed 's/[M ]//g')"
		umount "${mount_dir}"
		losetup -d "${device}"

		if [ "${avail}" -gt "${rootfs_size_mb}" ]; then
			rmdir "${mount_dir}"
			rm -f "${image}"
			echo "${img_size}"
			return
		fi
	done


	rmdir "${mount_dir}"
	rm -f "${image}"
	error "Could not calculate the required disk size"
}

# Calculate image size based on the rootfs and free space
calculate_img_size() {
	local rootfs="$1"
	local root_free_space_mb="$2"
	local fs_type="$3"
	local block_size="$4"

	# rootfs start + DAX header size + rootfs end
	local reserved_size_mb=$((rootfs_start + dax_header_sz + rootfs_end))

	disk_size="$(calculate_required_disk_size "${rootfs}" "${fs_type}" "${block_size}")"

	img_size="$((disk_size + reserved_size_mb))"
	if [ -n "${root_free_space_mb}" ]; then
		img_size="$((img_size + root_free_space_mb))"
	fi

	remaining="$((img_size % mem_boundary_mb))"
	if [ "${remaining}" != "0" ]; then
		img_size=$((img_size + mem_boundary_mb - remaining))
	fi

	echo "${img_size}"
}

setup_loop_device() {
	local image="$1"

	# Get the loop device bound to the image file (requires /dev mounted in the
	# image build system and root privileges)
	device=$(losetup -P -f --show "${image}")

	#Refresh partition table
	partprobe -s "${device}" > /dev/null
	# Poll for the block device p1
	for _ in $(seq 1 5); do
		if [ -b "${device}p1" ]; then
			echo "${device}"
			return 0
		fi
		sleep 1
	done

	error "File ${device}p1 is not a block device"
	return 1
}

format_loop() {
	local device="$1"
	local block_size="$2"
	local fs_type="$3"
	local mount_dir="$4"

	case "${fs_type}" in
		"${ext4_format}")
			mkfs.ext4 -q -F -b "${block_size}" "${device}p1"
			info "Set filesystem reserved blocks percentage to ${reserved_blocks_percentage}%"
			tune2fs -m "${reserved_blocks_percentage}" "${device}p1"
      return 0
			;;

		"${xfs_format}")
			# DAX and reflink cannot be used together!
			# Explicitly disable reflink, if it fails then reflink
			# is not supported and '-m reflink=0' is not needed.
			if mkfs.xfs -m reflink=0 -q -f -b size="${block_size}" "${device}p1" 2>&1 | grep -q "unknown option"; then
				mkfs.xfs -q -f -b size="${block_size}" "${device}p1"
			fi
      return 0
    	;;

		*)
			error "Unsupported fs type: ${fs_type}"
			return 1
			;;
	esac
}

create_disk() {
	local image="$1"
	local img_size="$2"
	local fs_type="$3"
	local part_start="$4"

	info "Creating raw disk with size ${img_size}M"
	qemu-img create -q -f raw "${image}" "${img_size}M"
	OK "Image file created"

	# Kata runtime expect an image with just one partition
	# The partition is the rootfs content
	info "Creating partitions"

	if [ "${rootfs_end}" == "-1" ]; then
		rootfs_end_unit="s"
	else
		rootfs_end_unit="MiB"
	fi
	if [ "${MEASURED_ROOTFS}" == "yes" ]; then
		info "Creating partitions with hash device"
		# The hash data will take less than one percent disk space to store
		hash_start=$(echo $img_size | awk '{print $1 * 0.99}' |cut -d $(locale decimal_point) -f 1)
		partition_param="mkpart primary ${fs_type} ${part_start}MiB ${hash_start}MiB "
		partition_param+="mkpart primary ${fs_type} ${hash_start}MiB ${rootfs_end}${rootfs_end_unit} "
		partition_param+="set 1 boot on"
	else
		partition_param="mkpart primary ${fs_type} ${part_start}MiB ${rootfs_end}${rootfs_end_unit}"
	fi

	parted -s -a optimal "${image}" -- \
		   mklabel msdos \
		   "${partition_param}"

	OK "Partitions created"
}

setup_selinux() {
		local mount_dir="$1"
		local agent_bin="$2"

		if [ "${SELINUX}" == "yes" ]; then
			if [ "${AGENT_INIT}" == "yes" ]; then
				die "Guest SELinux with the agent init is not supported yet"
			fi

			info "Labeling rootfs for SELinux"
			selinuxfs_path="${mount_dir}${SELINUXFS}"
			mkdir -p "$selinuxfs_path"
			if mountpoint $SELINUXFS > /dev/null && \
				chroot "${mount_dir}" command -v restorecon > /dev/null; then
				mount -t selinuxfs selinuxfs "$selinuxfs_path"
				chroot "${mount_dir}" restorecon -RF -e ${SELINUXFS} /
				umount "${selinuxfs_path}"
			else
				die "Could not label the rootfs. Make sure that SELinux is enabled on the host \
  and the rootfs is built with SELINUX=yes"
			fi
		fi
}

setup_systemd() {
		info "Creating empty machine-id to allow systemd to bind-mount it"
		touch "${mount_dir}/etc/machine-id"
}

create_rootfs_image() {
	local rootfs="$1"
	local image="$2"
	local img_size="$3"
	local fs_type="$4"
	local block_size="$5"
	local agent_bin="$6"

	create_disk "${image}" "${img_size}" "${fs_type}" "${rootfs_start}"

	if ! device="$(setup_loop_device "${image}")"; then
		die "Could not setup loop device"
	fi

	if ! format_loop "${device}" "${block_size}" "${fs_type}" ""; then
		die "Could not format loop device: ${device}"
	fi

	info "Mounting root partition"
	local mount_dir=$(mktemp -p "${TMPDIR:-/tmp}" -d osbuilder-mount-dir.XXXX)
	mount "${device}p1" "${mount_dir}"
	OK "root partition mounted"

	info "Copying content from rootfs to root partition"
	cp -a "${rootfs}"/* "${mount_dir}"

	info "Setup SELinux"
	setup_selinux "${mount_dir}" "${agent_bin}"

	sync
	OK "rootfs copied"

	info "Setup systemd"
	setup_systemd "${mount_dir}"

	info "Unmounting root partition"
	umount "${mount_dir}"
	OK "Root partition unmounted"

	if [ "${fs_type}" = "${ext4_format}" ]; then
		fsck.ext4 -D -y "${device}p1"
	fi

	if [ "${MEASURED_ROOTFS}" == "yes" ] && [ -b "${device}p2" ]; then
		info "veritysetup format rootfs device: ${device}p1, hash device: ${device}p2"
		local image_dir=$(dirname "${image}")
		veritysetup format "${device}p1" "${device}p2" > "${image_dir}"/root_hash.txt 2>&1
	fi

	losetup -d "${device}"
	rm -rf "${mount_dir}"
}

create_erofs_rootfs_image() {
	local rootfs="$1"
	local image="$2"
	local block_size="$3"
	local agent_bin="$4"

	if [ "$block_size" -ne 4096 ]; then
		die "Invalid block size for erofs"
	fi

	if ! device="$(setup_loop_device "${image}")"; then
		die "Could not setup loop device"
	fi

	local mount_dir=$(mktemp -p "${TMPDIR:-/tmp}" -d osbuilder-mount-dir.XXXX)

	info "Copying content from rootfs to root partition"
	cp -a "${rootfs}"/* "${mount_dir}"

	info "Setup SELinux"
	setup_selinux "${mount_dir}" "${agent_bin}"

	sync
	OK "rootfs copied"

	info "Setup systemd"
	setup_systemd "${mount_dir}"

	readonly fsimage="$(mktemp)"
	mkfs.erofs -Enoinline_data "${fsimage}" "${mount_dir}"
	local img_size="$(stat -c"%s" "${fsimage}")"
	local img_size_mb="$(((("${img_size}" + 1048576) / 1048576) + 1 + "${rootfs_start}"))"

	create_disk "${image}" "${img_size_mb}" "ext4" "${rootfs_start}"

	dd if="${fsimage}" of="${device}p1"

	losetup -d "${device}"
	rm -rf "${mount_dir}"

	return "${img_size_mb}"
}

set_dax_header() {
	local image="$1"
	local img_size="$2"
	local fs_type="$3"
	local nsdax_bin="$4"

	# rootfs start + DAX header size
	local rootfs_offset=$((rootfs_start + dax_header_sz))
	local header_image="${image}.header"
	local dax_image="${image}.dax"
	rm -f "${dax_image}" "${header_image}"

	create_disk "${header_image}" "${img_size}" "${fs_type}" "${rootfs_offset}"

	dax_header_bytes=$((dax_header_sz * 1024 * 1024))
	dax_alignment_bytes=$((dax_alignment * 1024 * 1024))
	info "Set DAX metadata"
	# Set metadata header
	# Issue: https://github.com/kata-containers/osbuilder/issues/240
	if [ -z "${nsdax_bin}" ] ; then
		nsdax_bin="${script_dir}/nsdax"
		gcc -O2 "${script_dir}/nsdax.gpl.c" -o "${nsdax_bin}"
		trap "rm ${nsdax_bin}" EXIT
	fi
	"${nsdax_bin}" "${header_image}" "${dax_header_bytes}" "${dax_alignment_bytes}"
	sync

	touch "${dax_image}"
	# Copy MBR #1 + DAX metadata
	dd if="${header_image}" of="${dax_image}" bs="${dax_header_sz}M" count=1
	# Copy MBR #2 + Rootfs
	dd if="${image}" of="${dax_image}" oflag=append conv=notrunc
	# final image
	mv "${dax_image}" "${image}"
	sync

	rm -f "${dax_image}" "${header_image}"
}

main() {
	# variables that can be overwritten by environment variables
	local agent_bin="${AGENT_BIN:-kata-agent}"
	local agent_init="${AGENT_INIT:-no}"
	local fs_type="${FS_TYPE:-${ext4_format}}"
	local image="${IMAGE:-kata-containers.img}"
	local block_size="${BLOCK_SIZE:-4096}"
	local root_free_space="${ROOT_FREE_SPACE:-}"
	local nsdax_bin="${NSDAX_BIN:-}"

	while getopts "ho:r:f:" opt
	do
		case "$opt" in
			h)	usage; return 0;;
			o)	image="${OPTARG}" ;;
			r)	root_free_space="${OPTARG}" ;;
			f)	fs_type="${OPTARG}" ;;
			*) break ;;
		esac
	done

	shift $(( OPTIND - 1 ))
	rootfs="$(readlink -f "$1")"
	if [ -z "${rootfs}" ]; then
		usage
		exit 0
	fi

	local container_engine
	if [ -n "${USE_DOCKER}" ]; then
		container_engine="docker"
	elif [ -n "${USE_PODMAN}" ]; then
		container_engine="podman"
	fi

	if [ -n "$container_engine" ]; then
		build_with_container "${rootfs}" \
			"${image}" "${fs_type}" "${block_size}" \
			"${root_free_space}" "${agent_bin}" \
			"${agent_init}" "${container_engine}" \
			"${nsdax_bin}"
		exit $?
	fi

	if ! check_rootfs "${rootfs}" ; then
		die "Invalid rootfs"
	fi

	if [ "${fs_type}" == 'erofs' ]; then
		# mkfs.erofs accepts an src root dir directory as an input
		# rather than some device, so no need to guess the device dest size first.
		create_erofs_rootfs_image "${rootfs}" "${image}" \
						"${block_size}" "${agent_bin}"
		rootfs_img_size=$?
		img_size=$((rootfs_img_size + dax_header_sz))
	else
		img_size=$(calculate_img_size "${rootfs}" "${root_free_space}" \
			"${fs_type}" "${block_size}")

		# the first 2M are for the first MBR + NVDIMM metadata and were already
		# consider in calculate_img_size
		rootfs_img_size=$((img_size - dax_header_sz))
		create_rootfs_image "${rootfs}" "${image}" "${rootfs_img_size}" \
						"${fs_type}" "${block_size}" "${agent_bin}"
	fi
	# insert at the beginning of the image the MBR + DAX header
	set_dax_header "${image}" "${img_size}" "${fs_type}" "${nsdax_bin}"

	chown "${USER}:${GROUP}" "${image}"
}

main "$@"
