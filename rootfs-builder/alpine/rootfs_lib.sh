#!/bin/bash
#
# Copyright (c) 2018 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0

check_root()
{
	if [ "$(id -u)" != "0" ]; then
		echo "Root is needed"
		exit 1
	fi
}

# - Arguments
# rootfs_dir=$1
#
# - Optional environment variables
#
# EXTRA_PKGS: Variable to add extra PKGS provided by the user
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

	# In case of support EXTRA packages, use it to allow
	# users add more packages to the base rootfs
	local EXTRA_PKGS=${EXTRA_PKGS:-}

	# Populate ROOTFS_DIR
	check_root
	mkdir -p "${ROOTFS_DIR}"

	/sbin/apk.static \
	    -X ${MIRROR}/${OS_VERSION}/main \
	    -U \
	    --allow-untrusted \
	    --root ${ROOTFS_DIR}\
	    --initdb add ${BASE_PACKAGES} ${EXTRA_PKGS} ${PACKAGES}

	mkdir -p ${ROOTFS_DIR}{/root,/etc/apk,/proc}
	echo "${MIRROR}/${OS_VERSION}/main" >  ${ROOTFS_DIR}/etc/apk/repositories
}
