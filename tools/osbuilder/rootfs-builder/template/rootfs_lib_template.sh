#
# Copyright (c) 2018-2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# - Arguments
# rootfs_dir=$1
#
# - Optional environment variables
#
# EXTRA_PKGS: Variable to add extra PKGS provided by the user
#
# BIN_AGENT: Name of the Kata-Agent binary
#
# REPO_URL: URL to distribution repository ( should be configured in
#           config.sh file)
#
# Any other configuration variable for a specific distro must be added
# and documented on its own config.sh
#
# - Expected result
#
# rootfs_dir populated with rootfs pkgs
# It must provide a binary in /sbin/init
#
# Note: For some distros, the build_rootfs(), before_starting_container()
#       and after_starting_container() functions provided in scripts/lib.sh
#       will suffice. If a new distro is introduced with a special requirement,
#       then, a rootfs_builder/<distro>/rootfs_lib.sh file should be created
#       using this template.

build_rootfs() {
	# Mandatory
	local ROOTFS_DIR=$1

	#Name of the Kata-Agent binary
	local BIN_AGENT=${BIN_AGENT}

	# In case of support EXTRA packages, use it to allow
	# users add more packages to the base rootfs
	local EXTRA_PKGS=${EXTRA_PKGS:-}

	#In case rootfs is created usign repositories allow user to modify
	# the default URL
	local REPO_URL=${REPO_URL:-YOUR_REPO}

	#PATH where files this script is placed
	#Use it to refer to files in the same directory
	#Exmaple: ${CONFIG_DIR}/foo
	local CONFIG_DIR=${CONFIG_DIR}


	# Populate ROOTFS_DIR
	# Must provide /sbin/init and /bin/${BIN_AGENT}
}

before_starting_container() {
	# Run the following tasks before starting the container that builds the rootfs.
	# For example:
	# * Create a container
	# * Create a volume
	return 0
}

after_stopping_container() {
	# Run the following tasks after stoping the container that builds the rootfs.
	# For example:
	# * Delete a container
	# * Delete a volume
	return 0
}
