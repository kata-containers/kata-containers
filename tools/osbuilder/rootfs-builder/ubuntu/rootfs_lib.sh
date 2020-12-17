# - Arguments
#
# Copyright (c) 2018  Yash Jain
#
# SPDX-License-Identifier: Apache-2.0
#
#
# rootfs_dir=$1
#
# - Optional environment variables
#
# EXTRA_PKGS: Variable to add extra PKGS provided by the user
#
# BIN_AGENT: Name of the Kata-Agent binary
#
# REPO_URL: URL to distribution repository ( should be configured in
#			config.sh file)
#
# Any other configuration variable for a specific distro must be added
# and documented on its own config.sh
#
# - Expected result
#
# rootfs_dir populated with rootfs pkgs
# It must provide a binary in /sbin/init
#
build_rootfs() {
	# Mandatory
	local ROOTFS_DIR=$1

	# Name of the Kata-Agent binary
	local BIN_AGENT=${BIN_AGENT}

	# In case of support EXTRA packages, use it to allow
	# users to add more packages to the base rootfs
	local EXTRA_PKGS=${EXTRA_PKGS:-}

	# In case rootfs is created using repositories allow user to modify
	# the default URL
	local REPO_URL=${REPO_URL:-YOUR_REPO}

	# PATH where files this script is placed
	# Use it to refer to files in the same directory
	# Example: ${CONFIG_DIR}/foo
	local CONFIG_DIR=${CONFIG_DIR}


	# Populate ROOTFS_DIR
	# Must provide /sbin/init and /bin/${BIN_AGENT}
	DEBOOTSTRAP="debootstrap"
	check_root
	mkdir -p "${ROOTFS_DIR}"
	if [ -n "${PKG_MANAGER}"  ]; then
		info "debootstrap path provided by user: ${PKG_MANAGER}"
	elif check_program $DEBOOTSTRAP ; then
		PKG_MANAGER=$DEBOOTSTRAP
	else
		die "$DEBOOTSTRAP is not installed"
	fi
	# trim whitespace
	PACKAGES=$(echo $PACKAGES |xargs )
	EXTRA_PKGS=$(echo $EXTRA_PKGS |xargs)
	# add comma as debootstrap needs , separated package names.
	# Don't change $PACKAGES in config.sh to include ','
	# This is done to maintain consistency
	PACKAGES=$(echo $PACKAGES | sed  -e 's/ /,/g' )
	EXTRA_PKGS=$(echo $EXTRA_PKGS | sed  -e 's/ /,/g' )

	# extra packages are added to packages and finally passed to debootstrap
	if [ "${EXTRA_PKGS}" = ""  ]; then
		echo "no extra packages"
	else
		PACKAGES="${PACKAGES},${EXTRA_PKGS}"
	fi

	${PKG_MANAGER} --variant=minbase \
		--arch=${ARCHITECTURE}\
		--include="$PACKAGES" \
		${OS_NAME} \
		${ROOTFS_DIR}

	chroot $ROOTFS_DIR ln -s /lib/systemd/systemd /usr/lib/systemd/systemd

    # Reduce image size and memory footprint
    # removing not needed files and directories.
    chroot $ROOTFS_DIR rm -rf /usr/share/{bash-completion,bug,doc,info,lintian,locale,man,menu,misc,pixmaps,terminfo,zoneinfo,zsh}
}
