#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

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
	REPO_NAME=${REPO_NAME:-"base"}
	CACHE_DIR=${CACHE_DIR:-"/var/cache/dnf-${OS_NAME}"}
	cat > "${DNF_CONF}" << EOF
[main]
cachedir=${CACHE_DIR}
logfile=${LOG_FILE}
keepcache=0
debuglevel=2
exactarch=1
obsoletes=1
plugins=0
installonly_limit=3
reposdir=/root/mash
retries=5
EOF
	if [ "$BASE_URL" != "" ]; then
            cat >> "${DNF_CONF}" << EOF

[base]
name=${OS_NAME}-${OS_VERSION} ${REPO_NAME}
failovermethod=priority
baseurl=${BASE_URL}
enabled=1
EOF
	elif [ "$MIRROR_LIST" != "" ]; then
	    cat >> "${DNF_CONF}" << EOF

[base]
name=${OS_NAME}-${OS_VERSION} ${REPO_NAME}
mirrorlist=${MIRROR_LIST}
enabled=1
EOF
	fi

	if [ "$GPG_KEY_FILE" != "" ]; then
            cat >> "${DNF_CONF}" << EOF
gpgcheck=1
gpgkey=file://${CONFIG_DIR}/${GPG_KEY_FILE}

EOF
	fi

}

build_rootfs()
{
	# Mandatory
	local ROOTFS_DIR="$1"

	# In case of support EXTRA packages, use it to allow
	# users add more packages to the base rootfs
	local EXTRA_PKGS=${EXTRA_PKGS:-""}

	#PATH where files this script is placed
	#Use it to refer to files in the same directory
	#Exmaple: ${CONFIG_DIR}/foo
	#local CONFIG_DIR=${CONFIG_DIR}

	check_root
	if [ ! -f "${DNF_CONF}" ]; then
		DNF_CONF="./kata-${OS_NAME}-dnf.conf"
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

	DNF="${PKG_MANAGER} --config=$DNF_CONF -y --installroot=${ROOTFS_DIR} --noplugins"
	$DNF install ${EXTRA_PKGS} ${PACKAGES}

	[ -n "${ROOTFS_DIR}" ]  && rm -r "${ROOTFS_DIR}${CACHE_DIR}"
}
