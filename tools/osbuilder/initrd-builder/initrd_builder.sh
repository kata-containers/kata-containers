#!/usr/bin/env bash
#
# Copyright (c) 2018 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0

[ -z "${DEBUG}" ] || set -x

set -o errexit
# set -o nounset
set -o pipefail

set -x

script_name="${0##*/}"
script_dir="$(dirname $(readlink -f $0))"

lib_file="${script_dir}/../scripts/lib.sh"
source "$lib_file"

INITRD_IMAGE="${INITRD_IMAGE:-kata-containers-initrd.img}"
AGENT_BIN=${AGENT_BIN:-kata-agent}
AGENT_INIT=${AGENT_INIT:-no}

# The list of systemd units and files that are not needed in Kata Containers
readonly -a systemd_units=(
	"systemd-coredump@"
	"systemd-journald"
	"systemd-journald-dev-log"
	"systemd-journal-flush"
	"systemd-random-seed"
	"systemd-timesyncd"
	"systemd-tmpfiles-setup"
	"systemd-update-utmp"
#	"systemd-udevd"
#	"systemd-udevd-control"
#	"systemd-udevd-kernel"
#	"systemd-udev-trigger"
	"initrd-cleanup.service"
	"initrd-udevadm-cleanup-db.service"
	"initrd-switch-root.service"
)

readonly -a systemd_files=(
	"systemd-bless-boot-generator"
	"systemd-fstab-generator"
	"systemd-getty-generator"
	"systemd-gpt-auto-generator"
	"systemd-tmpfiles-cleanup.timer"
)

setup_systemd() {
		local mount_dir="$1"

		info "Removing unneeded systemd services and sockets"
		for u in "${systemd_units[@]}"; do
			find "${mount_dir}" -type f \( \
				 -name "${u}.service" -o \
				 -name "${u}.socket" \) \
				 -exec rm -f {} \;
		done

		info "Removing unneeded systemd files"
		for u in "${systemd_files[@]}"; do
			find "${mount_dir}" -type f -name "${u}" -exec rm -f {} \;
		done

		info "Creating empty machine-id to allow systemd to bind-mount it"
		touch "${mount_dir}/etc/machine-id"
}


usage()
{
	error="${1:-0}"
	cat <<EOF
Usage: ${script_name} [options] <rootfs-dir>
	This script creates a Kata Containers initrd image file based on the
	<rootfs-dir> directory.

Options:
	-h Show help
	-o Set the path where the generated image file is stored.
	   DEFAULT: the path stored in the environment variable INITRD_IMAGE

Extra environment variables:
	AGENT_BIN:  use it to change the expected agent binary name
		    DEFAULT: kata-agent
	AGENT_INIT: use kata agent as init process
		    DEFAULT: no
EOF
exit "${error}"
}

while getopts "ho:" opt
do
	case "$opt" in
		h)	usage ;;
		o)	INITRD_IMAGE="${OPTARG}" ;;
	esac
done

shift $(( $OPTIND - 1 ))

ROOTFS="$1"


[ -n "${ROOTFS}" ] || usage
[ -d "${ROOTFS}" ] || die "${ROOTFS} is not a directory"

ROOTFS=$(readlink -f ${ROOTFS})
IMAGE_DIR=$(dirname ${INITRD_IMAGE})
IMAGE_DIR=$(readlink -f ${IMAGE_DIR})
IMAGE_NAME=$(basename ${INITRD_IMAGE})

# The kata rootfs image expects init to be installed
init="${ROOTFS}/sbin/init"
[ -x "${init}" ] || [ -L ${init} ] || die "/sbin/init is not installed in ${ROOTFS}"
OK "init is installed"
[ "${AGENT_INIT}" == "yes" ] || [ -x "${ROOTFS}/usr/bin/${AGENT_BIN}" ] || \
	die "/usr/bin/${AGENT_BIN} is not installed in ${ROOTFS}
	use AGENT_BIN env variable to change the expected agent binary name"
OK "Agent is installed"

# initramfs expects /init
#ln -sf /sbin/init  "${ROOTFS}/init"
# For the gpu use-case we're creating our own init script not reyling on systemd

cat <<-'CHROOT_EOF' > "${ROOTFS}/init"
	#!/bin/bash -x

#	> /etc/ld.so.cache
#	ldconfig

	/usr/lib/systemd/systemd-udevd --daemon --resolve-names=never

	modprobe nvidia
	ls -l /dev/nvidia*


	exec /usr/bin/kata-agent
CHROOT_EOF

OK "init script created"
cat ${ROOTFS}/init

OK "make executable"
chmod +x "${ROOTFS}/init"

# create an initrd-release file systemd uses the existence of this file as a 
# flag whether to run in initial RAM disk mode, or not.
#cp "${ROOTFS}/etc/os-release" "${ROOTFS}/etc/initrd-release"

#OK "Systemd Setup"

#setup_systemd "${ROOTFS}"


info "Creating ${IMAGE_DIR}/${IMAGE_NAME} based on rootfs at ${ROOTFS}"
( cd "${ROOTFS}" && find . | cpio -H newc -o | pigz -9 ) > "${IMAGE_DIR}"/"${IMAGE_NAME}"
