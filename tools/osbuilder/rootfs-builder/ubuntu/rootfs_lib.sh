#!/usr/bin/env bash
#
# Copyright (c) 2018 Yash Jain, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

build_rootfs() {
	# Several steps below write to paths derived from this one, one of them the
	# resolver: an empty rootfs_dir would target the build machine's own /etc.
	local rootfs_dir="${1:?rootfs_dir is required}"
	[[ "${rootfs_dir}" != "/" ]] || die "refusing to build a rootfs at /"

	# This fixes the spurious error
	# E: Can't find a source to download version '2021.03.26' of 'ubuntu-keyring:amd64'
	apt update
	# focal version of mmdebstrap only supports comma separated package lists
	# shellcheck disable=SC2154
	if [[ "${OS_VERSION}" = "focal" ]]; then
		PACKAGES=$(echo "${PACKAGES}" | tr ' ' ',')
		EXTRA_PKGS=$(echo "${EXTRA_PKGS}" | tr ' ' ',')
	fi
	# shellcheck disable=SC2154
	if ! mmdebstrap --mode auto --arch "${DEB_ARCH}" --variant required \
			--components="${REPO_COMPONENTS}" \
			--include "${PACKAGES},${EXTRA_PKGS}" "${OS_VERSION}" "${rootfs_dir}" "${REPO_URL}"; then
		echo "ERROR: mmdebstrap failed, cannot proceed" && exit 1
	else
		echo "INFO: mmdebstrap succeeded"
	fi
	rm -rf "${rootfs_dir}/var/run"
	ln -s /run "${rootfs_dir}/var/run"
	cp --remove-destination /etc/resolv.conf "${rootfs_dir}/etc"

	local dir="${rootfs_dir}/etc/ssl/certs"
	mkdir -p "${dir}"
	cp --remove-destination /etc/ssl/certs/ca-certificates.crt "${dir}"

	# apt verifies TLS through OpenSSL, which only consults ${OPENSSLDIR}/cert.pem
	# and ${OPENSSLDIR}/certs. Neither exists here because the bundle above is
	# copied in rather than installed via the openssl package, so point OpenSSL's
	# default CAfile at it.
	mkdir -p "${rootfs_dir}/usr/lib/ssl"
	ln -sf ../../../etc/ssl/certs/ca-certificates.crt \
		"${rootfs_dir}/usr/lib/ssl/cert.pem"

	# Reduce image size and memory footprint by removing unnecessary files and directories.
	rm -rf "${rootfs_dir}"/usr/share/{bash-completion,bug,doc,info,lintian,locale,man,menu,misc,pixmaps,terminfo,zsh}

	# Minimal set of device nodes needed when AGENT_INIT=yes so that the
	# kernel can properly setup stdout/stdin/stderr for us
	pushd "${rootfs_dir}/dev" || return
	MAKEDEV -v console tty ttyS null zero fd
	popd || return
}
