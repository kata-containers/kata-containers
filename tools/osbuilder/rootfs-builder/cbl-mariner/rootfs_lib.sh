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

	# kata-packages-uvm pulls in ca-certificates, so the guest gets Azure Linux's
	# own trust store rather than the build machine's bundle, and that store
	# still trusts compromised_ca_cns. Blocklisting them makes update-ca-trust
	# drop them from every bundle it extracts. p11-kit labels are distro-specific
	# ("Sectigo (AAA)"), so match on the subject of each anchor instead.
	local anchors="/tmp/kata-ca-anchors" cert blocked=no
	rm -rf "${ROOTFS_DIR:?}${anchors}"
	chroot "${ROOTFS_DIR}" trust extract --format=pem-directory --filter=ca-anchors "${anchors}"
	for cert in "${ROOTFS_DIR}${anchors}"/*.pem; do
		is_compromised_ca_cert "${cert}" || continue
		info "distrusting compromised CA certificate $(basename "${cert}")"
		cp "${cert}" "${ROOTFS_DIR}/etc/pki/ca-trust/source/blocklist/"
		blocked=yes
	done
	rm -rf "${ROOTFS_DIR:?}${anchors}"
	[[ "${blocked}" == no ]] || chroot "${ROOTFS_DIR}" update-ca-trust extract

	# etc/pki/tls/certs/ca-bundle.crt is an absolute symlink, so check its target
	# inside the rootfs instead of the build machine's.
	check_ca_bundle "${ROOTFS_DIR}/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem"

	rm -rf "${ROOTFS_DIR}"/usr/share/{bash-completion,cracklib,doc,info,locale,man,misc,pixmaps,terminfo,zoneinfo,zsh}
}
