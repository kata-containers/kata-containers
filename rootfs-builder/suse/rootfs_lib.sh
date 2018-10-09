#
# Copyright (c) 2018 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0

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
# Note: For some distros, the build_rootfs() function provided in scripts/lib.sh
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

	#PATH where files this script is placed
	#Use it to refer to files in the same directory
	#Exmaple: ${CONFIG_DIR}/foo
	local CONFIG_DIR=${CONFIG_DIR}

	# Populate ROOTFS_DIR
	# Must provide /sbin/init and /bin/${BIN_AGENT}
	if [ -e "$ROOTFS_DIR" ] && ! [ -z "$(ls -A $ROOTFS_DIR)" ]; then
		echo "ERROR: $ROOTFS_DIR is not empty"
		exit 1
	fi

	local addPackages=""
	for p in $PACKAGES $EXTRA_PKGS; do
		addPackages+=" --add-package=$p"
	done

	# set-repo format: <source,type,alias,priority,imageinclude,package_gpgcheck>
	# man kiwi::system::build for details
	local setRepo=" --set-repo $REPO_URL,rpm-md,$OS_IDENTIFIER,99,false,false"

	# Workaround for zypper slowdowns observed when running inside
	# a container: see https://github.com/openSUSE/zypper/pull/209
	# The fix is upstream but it will take a while before landing
	# in Leap
	ulimit -n 1024
	kiwi system prepare \
		--description $CONFIG_DIR \
		--allow-existing-root \
		--root $ROOTFS_DIR \
		$addPackages \
		$setRepo
	install -d $ROOTFS_DIR/lib/systemd
	ln -s /usr/lib/systemd/systemd $ROOTFS_DIR/lib/systemd/systemd
}
