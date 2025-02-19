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
	read -r -a packages <<< "${PACKAGES} ${EXTRA_PKGS}"
	IFS=,
	local comma_separated_packages="${packages[*]}"
	unset IFS

	debootstrap --arch=amd64 --variant=minbase --include=${comma_separated_packages} --components=main,universe \
		noble ${rootfs_dir} http://us.archive.ubuntu.com/ubuntu/

	ret=$?
	if [ ${ret} -ne 0 ]; then
		echo "FAILED TO BUILD ROOTFS. DEBOOTSTRAP return ${ret}"
  		exit ${ret}
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

	local script_dir=$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")
	source ${script_dir}/superprotocol/postbuild.sh
	run_postbuild ${rootfs_dir}
}
