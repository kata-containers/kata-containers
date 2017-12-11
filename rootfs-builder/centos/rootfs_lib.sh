#!/bin/bash
#
# Copyright (c) 2017 Intel Corporation
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

generate_dnf_config()
{
	cat > "${DNF_CONF}" << EOF
[main]
cachedir=/var/cache/centos-osbuilder
keepcache=0
debuglevel=2
logfile=/var/log/yum-centos.log
exactarch=1
obsoletes=1
gpgcheck=0
plugins=0
installonly_limit=3
#Dont use the default dnf reposdir
#this will prevent to use host repositories
reposdir=/root/mash

[base]
name=CentOS-7 - Base
mirrorlist=http://mirrorlist.centos.org/?release=7&arch=x86_64&repo=os&container=container
#baseurl=${REPO_URL}/os/x86_64/
gpgcheck=1
gpgkey=file://${CONFIG_DIR}/RPM-GPG-KEY-CentOS-7

#released updates 
[updates]
name=CentOS-7 - Updates
mirrorlist=http://mirrorlist.centos.org/?release=7&arch=x86_64&repo=updates&container=container
#baseurl=${REPO_URL}/updates/x86_64/
gpgcheck=1
gpgkey=file://${CONFIG_DIR}/RPM-GPG-KEY-CentOS-7

#additional packages that may be useful
[extras]
name=CentOS-7 - Extras
mirrorlist=http://mirrorlist.centos.org/?release=7&arch=x86_64&repo=extras&container=container
#baseurl=${REPO_URL}/extras/x86_64/
gpgcheck=1
gpgkey=file://${CONFIG_DIR}/RPM-GPG-KEY-CentOS-7

#additional packages that extend functionality of existing packages
[centosplus]
name=CentOS-7 - Plus
mirrorlist=http://mirrorlist.centos.org/?release=7&arch=x86_64&repo=centosplus&container=container
#baseurl=${REPO_URL}/centosplus/x86_64/
gpgcheck=1
enabled=0
gpgkey=file://${CONFIG_DIR}/RPM-GPG-KEY-CentOS-7
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
	local REPO_URL=${REPO_URL:-http://mirror.centos.org/centos/7}

	#PATH where files this script is placed
	#Use it to refer to files in the same directory
	#Exmaple: ${CONFIG_DIR}/foo
	local CONFIG_DIR=${CONFIG_DIR}


	# Populate ROOTFS_DIR
	# Must provide /sbin/init and /bin/${BIN_AGENT}
	check_root
	if [ ! -f "${DNF_CONF}" ]; then
		DNF_CONF="./kata-centos-dnf.conf"
		generate_dnf_config
	fi
	mkdir -p "${ROOTFS_DIR}"
	if [ -n "${PKG_MANAGER}" ]; then
		info "DNF path provided by user: ${PKG_MANAGER}"
	elif check_program "dnf"; then
		PKG_MANAGER="dnf"
	elif check_program "yum" ; then
		PKG_MANAGER="yum"
	else
		die "neither yum nor dnf is installed"
	fi

	info "Using : ${PKG_MANAGER} to pull packages from ${REPO_URL}"

	DNF="${PKG_MANAGER} --config=$DNF_CONF -y --installroot=${ROOTFS_DIR} --noplugins"
	$DNF install ${EXTRA_PKGS} ${PACKAGES}

	[ -n "${ROOTFS_DIR}" ]  && rm -r "${ROOTFS_DIR}/var/cache/centos-osbuilder"
}
