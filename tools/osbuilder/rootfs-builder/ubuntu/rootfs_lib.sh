# Copyright (c) 2018 Yash Jain, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

build_rootfs() {
	local rootfs_dir=$1
	local multistrap_conf=multistrap.conf

	[ -z "$rootfs_dir" ] && die "need rootfs"
	[ "$rootfs_dir" = "/" ] && die "rootfs cannot be slash"

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
	for file in /etc/{resolv.conf,ssl/certs/ca-certificates.crt}; do
		mkdir -p "$rootfs_dir$(dirname $file)"
		cp --remove-destination "$file" "$rootfs_dir$file"
	done

	# Reduce image size and memory footprint by removing unnecessary files and directories.
	rm -rf $rootfs_dir/usr/share/{bash-completion,bug,doc,info,lintian,locale,man,menu,misc,pixmaps,terminfo,zsh}

	if [ "${AA_KBC}" == "eaa_kbc" ] && [ "${ARCH}" == "x86_64" ]; then
		source /etc/os-release

		if [ "${VERSION_ID}" == "20.04" ]; then
			curl -L http://mirrors.openanolis.cn/inclavare-containers/ubuntu${VERSION_ID}/DEB-GPG-KEY.key | chroot "$rootfs_dir" apt-key add -
			cat << EOF | chroot "$rootfs_dir"
echo 'deb [arch=amd64] http://mirrors.openanolis.cn/inclavare-containers/ubuntu${VERSION_ID} focal main' | tee /etc/apt/sources.list.d/inclavare-containers.list
apt-get update
apt-get install -y rats-tls
EOF
		else
			echo "rats-tls is only provided for Ubuntu 20.04, there's yet no packages for Ubuntu ${VERSION_ID}"
		fi
	fi
}
