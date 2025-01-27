# Copyright (c) 2018 Yash Jain, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

build_dbus() {
	local rootfs_dir=$1
	ln -sf /lib/systemd/system/dbus.service $rootfs_dir/etc/systemd/system/dbus.service
	ln -sf /lib/systemd/system/dbus.socket $rootfs_dir/etc/systemd/system/dbus.socket
}

build_rootfs() {
	local rootfs_dir=$1
	local multistrap_conf=multistrap.conf

	# For simplicity's sake, use multistrap for foreign and native bootstraps.
	cat > "$multistrap_conf" << EOF
[General]
cleanup=true
aptsources=Ubuntu
bootstrap=Ubuntu

[Ubuntu]
source=$REPO_URL
keyring=ubuntu-keyring
suite=$OS_VERSION
packages=$PACKAGES $EXTRA_PKGS
EOF

	if [ "${CONFIDENTIAL_GUEST}" == "yes" ] && [ "${DEB_ARCH}" == "amd64" ]; then
		mkdir -p $rootfs_dir/etc/apt/trusted.gpg.d/
		curl -fsSL https://download.01.org/intel-sgx/sgx_repo/ubuntu/intel-sgx-deb.key |
			gpg --dearmour -o $rootfs_dir/etc/apt/trusted.gpg.d/intel-sgx-deb.gpg
		sed -i -e "s/bootstrap=Ubuntu/bootstrap=Ubuntu intel-sgx/" $multistrap_conf
		SUITE=$OS_VERSION
		# Intel does not release sgx stuff for non-LTS, thus if using oracular (24.10),
		# we need to enforce getting libtdx-attest from noble.
		[ "$SUITE" = "oracular" ] && SUITE="noble"
		cat >> $multistrap_conf << EOF

[intel-sgx]
source=https://download.01.org/intel-sgx/sgx_repo/ubuntu
suite=$SUITE
packages=libtdx-attest=1.22\*
EOF
	fi

	# This fixes the spurious error
	# E: Can't find a source to download version '2021.03.26' of 'ubuntu-keyring:amd64'
	apt update

	if ! multistrap -a "$DEB_ARCH" -d "$rootfs_dir" -f "$multistrap_conf"; then
		if [ "$OS_VERSION" = "focal" ]; then	
			echo "WARN: multistrap failed, proceed with hack for Ubuntu 20.04"
			build_dbus $rootfs_dir
		else
			echo "ERROR: multistrap failed, cannot proceed" && exit 1
		fi
	else
		echo "INFO: multistrap succeeded"
	fi
	rm -rf "$rootfs_dir/var/run"
	ln -s /run "$rootfs_dir/var/run"
	cp --remove-destination /etc/resolv.conf "$rootfs_dir/etc"

	local dir="$rootfs_dir/etc/ssl/certs"
	mkdir -p "$dir"
	cp --remove-destination /etc/ssl/certs/ca-certificates.crt "$dir"

	# Reduce image size and memory footprint by removing unnecessary files and directories.
	rm -rf $rootfs_dir/usr/share/{bash-completion,bug,doc,info,lintian,locale,man,menu,misc,pixmaps,terminfo,zsh}

	# Minimal set of device nodes needed when AGENT_INIT=yes so that the
	# kernel can properly setup stdout/stdin/stderr for us
	pushd $rootfs_dir/dev
	MAKEDEV -v console tty ttyS null zero fd
	popd
}
