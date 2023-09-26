#!/bin/bash -x

# Copyright (c) 2018 Yash Jain, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

build_dbus() {
	local rootfs_dir=$1

	ln -sf /lib/systemd/system/dbus.service "$rootfs_dir"/etc/systemd/system/dbus.service
	ln -sf /lib/systemd/system/dbus.socket "$rootfs_dir"/etc/systemd/system/dbus.socket
}

build_rootfs() {
	local rootfs_dir=$1
	local multistrap_conf=multistrap.conf
	local multistrap_log=multistrap.log

	# For simplicity's sake, use multistrap for foreign and native bootstraps.
	cat <<-EOF > "$multistrap_conf"
	[General]
	cleanup=true
	aptsources=Ubuntu
	bootstrap=Ubuntu

	[Ubuntu]
	source=$REPO_URL
	omitdebsrc=true
	keyring=ubuntu-keyring
	suite=${OS_VERSION:-focal}
	packages=$PACKAGES $EXTRA_PKGS
	EOF

	# Regenerate the apt sources list, multistrap can fail if the outer
	# environment has not been updated and ubuntu-keyring cannot be found.	
	apt-get update
	
	# setup of dbus needs the urandom device before multistrap is creating
	# the ddefault /dev entries
	mkdir "$rootfs_dir"/dev
	mknod "$rootfs_dir"/dev/urandom c 1 9

	multistrap -a "${DEB_ARCH}" -d "${rootfs_dir}" -f "${multistrap_conf}"
	# For SBOM generation we need the initial set of packages from multistrap
	echo "${PACKAGES} ${EXTRA_PKGS}" > "${rootfs_dir}/${multistrap_log}"

	build_dbus "$rootfs_dir"

	rm -rf "$rootfs_dir/var/run"
	ln -s /run "$rootfs_dir/var/run"
	cp --remove-destination /etc/resolv.conf "$rootfs_dir/etc"

	# Reduce image size and memory footprint by removing unnecessary files and directories.
	rm -rf "${rootfs_dir}"/usr/share/{bash-completion,bug,doc,info,lintian,locale,man,menu,misc,pixmaps,terminfo,zsh}

	# Minimal set of device nodes needed when AGENT_INIT=yes so that the
	# kernel can properly setup stdout/stdin/stderr for us
	pushd "${rootfs_dir}"/dev || exit >> /dev/null
	MAKEDEV -v console tty ttyS null zero fd
	popd || exit >> /dev/null
}
