# Copyright (c) 2018 Yash Jain, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

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
suite=focal
packages=$PACKAGES $EXTRA_PKGS
EOF
	multistrap -a "$DEB_ARCH" -d "$rootfs_dir" -f "$multistrap_conf"
	rm -rf "$rootfs_dir/var/run"
	ln -s /run "$rootfs_dir/var/run"
	cp --remove-destination /etc/resolv.conf "$rootfs_dir/etc"

	# Reduce image size and memory footprint by removing unnecessary files and directories.
	rm -rf $rootfs_dir/usr/share/{bash-completion,bug,doc,info,lintian,locale,man,menu,misc,pixmaps,terminfo,zsh}

	if [ "${AA_KBC}" == "eaa_kbc" ] && [ "${ARCH}" == "x86_64" ]; then
		wget -qO - http://mirrors.openanolis.cn/inclavare-containers/ubuntu20.04/DEB-GPG-KEY.key  | chroot $ROOTFS_DIR apt-key add -
		cat << EOF | chroot $ROOTFS_DIR
echo 'deb [arch=amd64] http://mirrors.openanolis.cn/inclavare-containers/ubuntu20.04 bionic main' | tee /etc/apt/sources.list.d/inclavare-containers.list
apt-get update
apt-get install -y rats-tls
EOF
	fi
}
