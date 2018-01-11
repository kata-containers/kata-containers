#!/bin/bash
#
# Copyright (c) 2018 Huawei Technologies Co., Ltd
#
# SPDX-License-Identifier: Apache-2.0

check_program(){
	type "$1" >/dev/null 2>&1
}

check_root()
{
	if [ "$(id -u)" != "0" ]; then
		echo "Root is needed"
		exit 1
	fi
}

generate_yum_config()
{
	cat > "${DNF_CONF}" << EOF
[main]
cachedir=/var/cache/euleros-osbuilder
keepcache=0
debuglevel=2
logfile=/var/log/yum-euleros.log
exactarch=1

[Base]
name=EulerOS-2.2 Base
baseurl=http://developer.huawei.com/ict/site-euleros/euleros/repo/yum/2.2/os/x86_64/
enabled=1
gpgcheck=1
gpgkey=file://${CONFIG_DIR}/RPM-GPG-KEY-EulerOS
EOF
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
	local REPO_URL=${REPO_URL:-http://developer.huawei.com/ict/site-euleros/euleros/repo/yum/2.2}

	#PATH where files this script is placed
	#Use it to refer to files in the same directory
	#Exmaple: ${CONFIG_DIR}/foo
	local CONFIG_DIR=${CONFIG_DIR}


	# Populate ROOTFS_DIR
	# Must provide /sbin/init and /bin/${BIN_AGENT}
	check_root
	if [ ! -f "{DNF_CONF}" ]; then
		DNF_CONF="./kata-euleros-yum.repo"
		generate_yum_config
	fi
	mkdir -p "${ROOTFS_DIR}"
	if [ -n "${PKG_MANAGER}" ]; then
		info "DNF path provided by user: ${PKG_MANAGER}"
	elif check_program "yum" ; then
		PKG_MANAGER="yum"
	else
		die "yum is not installed"
	fi

	info "Using : ${PKG_MANAGER} to pull packages from ${REPO_URL}"

	DNF="${PKG_MANAGER} --config=$DNF_CONF -y --installroot=${ROOTFS_DIR} --noplugins"
	$DNF install ${EXTRA_PKGS} ${PACKAGES}

	[ -n "${ROOTFS_DIR}" ]  && rm -r "${ROOTFS_DIR}/var/cache/euleros-osbuilder"
}
