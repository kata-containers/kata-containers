#!/bin/bash
#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

check_program(){
	type "$1" >/dev/null 2>&1
}

generate_dnf_config()
{
	cat > "${DNF_CONF}" << EOF
[main]
cachedir=/var/cache/dnf/kata/
keepcache=0
debuglevel=2
logfile=/var/log/dnf.log
exactarch=1
obsoletes=1
gpgcheck=0
plugins=0
installonly_limit=3
#Dont use the default dnf reposdir
#this will prevent to use host repositories
reposdir=/root/mash

[kata]
name=fedora
failovermethod=priority
baseurl=${REPO_URL}
enabled=1
gpgcheck=0
EOF
}

build_rootfs()
{
	# Mandatory
	local ROOTFS_DIR=$1

	#In case rootfs is created usig repositories allow user to modify
	# the default URL
	local REPO_URL=${REPO_URL:-http://mirror.math.princeton.edu/pub/fedora/linux/releases/$OS_VERSION/Everything/x86_64/os/}

	# In case of support EXTRA packages, use it to allow
	# users add more packages to the base rootfs
	local EXTRA_PKGS=${EXTRA_PKGS:-""}

	#PATH where files this script is placed
	#Use it to refer to files in the same directory
	#Exmaple: ${CONFIG_DIR}/foo
	#local CONFIG_DIR=${CONFIG_DIR}

	check_root
	if [ ! -f "${DNF_CONF}" ]; then
		DNF_CONF="./kata-fedora-dnf.conf"
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

	[ -n "${ROOTFS_DIR}" ]  && rm -r "${ROOTFS_DIR}/var/cache/dnf"
}


check_root()
{
	if [ "$(id -u)" != "0" ]; then
		echo "Root is needed"
		exit 1
	fi
}
