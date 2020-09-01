#!/bin/bash
#
# Copyright (c) 2018 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0

# - Arguments
# rootfs_dir=$1
#
# - Optional environment variables
#
# BIN_AGENT: Name of the Kata-Agent binary
#
# Any other configuration variable for a specific distro must be added
# and documented on its own config.sh
#
# - Expected result
#
# rootfs_dir populated with rootfs pkgs
# It must provide a binary in /sbin/init
build_rootfs() {
	# Mandatory
	local ROOTFS_DIR=$1

	# Populate ROOTFS_DIR
	check_root
	mkdir -p "${ROOTFS_DIR}"

	rm -rf ${ROOTFS_DIR}/var/tmp
	cp -a -r -f /bin /etc /lib /sbin /usr /var ${ROOTFS_DIR}
	mkdir -p ${ROOTFS_DIR}{/root,/proc,/dev,/home,/media,/mnt,/opt,/run,/srv,/sys,/tmp}

	echo "${MIRROR}/${OS_VERSION}/main" >  ${ROOTFS_DIR}/etc/apk/repositories
}
