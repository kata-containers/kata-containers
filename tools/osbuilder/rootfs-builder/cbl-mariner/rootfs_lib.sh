#!/usr/bin/env bash
#
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

build_rootfs()
{
	# Mandatory
	local ROOTFS_DIR="$1"

	[[ -z "${ROOTFS_DIR}" ]] && die "need rootfs"

	# In case of support EXTRA packages, use it to allow
	# users add more packages to the base rootfs
	local EXTRA_PKGS=${EXTRA_PKGS:-""}

	check_root
	mkdir -p "${ROOTFS_DIR}"
	PKG_MANAGER="tdnf"

	# shellcheck disable=SC2154
	DNF="${PKG_MANAGER} -y --installroot=${ROOTFS_DIR} --noplugins --releasever=${OS_VERSION}"

	info "install packages for rootfs"
	# shellcheck disable=SC2154,SC2086
	${DNF} install ${EXTRA_PKGS} ${PACKAGES}

	# cifs.upcall serves the kernel's cifs.spnego key requests from the ccache the agent writes for uid 0.
	mkdir -p "${ROOTFS_DIR}/etc/request-key.d"
	cat > "${ROOTFS_DIR}/etc/request-key.d/cifs.spnego.conf" <<EOF
create	cifs.spnego	*	*	/usr/sbin/cifs.upcall %k
EOF
	cat > "${ROOTFS_DIR}/etc/krb5.conf" <<EOF
[libdefaults]
	default_ccache_name = FILE:/run/kata-containers/krb5cc_%{uid}
EOF

	rm -rf "${ROOTFS_DIR}"/usr/share/{bash-completion,cracklib,doc,info,locale,man,misc,pixmaps,terminfo,zoneinfo,zsh}
}
