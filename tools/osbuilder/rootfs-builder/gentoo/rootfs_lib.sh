# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# - Arguments
# rootfs_dir=$1
#
# - Optional environment variables
#
# EXTRA_PKGS: Variable to add extra PKGS provided by the user
#
# BIN_AGENT: Name of the Kata-Agent binary
#
# REPO_URL: URL to distribution repository ( should be configured in
#			config.sh file)
#
# Any other configuration variable for a specific distro must be added
# and documented on its own config.sh
#
# - Expected result
#
# rootfs_dir populated with rootfs pkgs
# It must provide a binary in /sbin/init
#
gentoo_portage_container=gentoo_portage
gentoo_local_portage_dir="${HOME}/gentoo-$(date +%s)"

build_rootfs() {
	# Mandatory
	local ROOTFS_DIR=$1

	# In case of support EXTRA packages, use it to allow
	# users to add more packages to the base rootfs
	local EXTRA_PKGS=${EXTRA_PKGS:-}

	# Populate ROOTFS_DIR
	# Must provide /sbin/init and /bin/${BIN_AGENT}
	check_root
	mkdir -p "${ROOTFS_DIR}"

	# trim whitespace
	PACKAGES=$(echo $PACKAGES |xargs )
	EXTRA_PKGS=$(echo $EXTRA_PKGS |xargs)

	# extra packages are added to packages and finally passed to debootstrap
	if [ "${EXTRA_PKGS}" = ""  ]; then
		echo "no extra packages"
	else
		PACKAGES="${PACKAGES} ${EXTRA_PKGS}"
	fi

	local packageuseconf="/etc/portage/package.use/user"
	local makeconf="/etc/portage/make.conf"
	local systemd_optimizations=(
		acl
		-apparmor
		-audit
		cgroup-hybrid
		-cryptsetup
		-curl
		-dns-over-tls
		-gcrypt
		-gnuefi
		-homed
		-http
		-hwdb
		-idn
		-importd
		kmod
		-lz4
		-lzma
		-nat
		-pkcs11
		-policykit
		-pwquality
		-qrcode
		-repart
		-resolvconf
		sysv-utils
		-test
		-xkb
		-zstd
	)

	local packages_optimizations=(
		-abi_x86_32
		-abi_x86_x32
		-debug
		-doc
		-examples
		multicall
		-ncurses
		-nls
		-selinux
		systemd
		-udev
		-unicode
		-X
	)

	local compiler_optimizations=(
		-O3
		-fassociative-math
		-fasynchronous-unwind-tables
		-feliminate-unused-debug-types
		-fexceptions
		-ffat-lto-objects
		-fno-semantic-interposition
		-fno-signed-zeros
		-fno-trapping-math
		-fstack-protector
		-ftree-loop-distribute-patterns
		-m64
		-mtune=skylake
		--param=ssp-buffer-size=32
		-pipe
		-Wl,--copy-dt-needed-entries
		-Wp,-D_REENTRANT
		-Wl,--enable-new-dtags
		-Wl,-sort-common
		-Wl,-z -Wl,now
		-Wl,-z -Wl,relro
	)

	local build_dependencies=(
		dev-vcs/git
	)

	local conflicting_packages=(
		net-misc/netifrc sys-apps/sysvinit
		sys-fs/eudev sys-apps/openrc
		virtual/service-manager
	)

	# systemd optimizations
	echo "sys-apps/systemd ${systemd_optimizations[*]}" >> ${packageuseconf}
	echo "MAKEOPTS=\"-j$(nproc)\"" >> ${makeconf}

	# Packages optimizations
	echo "USE=\"${packages_optimizations[*]}\"" >> ${makeconf}

	# compiler optimizations
	echo "CFLAGS=\"${compiler_optimizations[*]}\"" >> ${makeconf}
	echo 'CXXFLAGS="${CFLAGS}"' >> ${makeconf}

	# remove conflicting packages
	emerge -Cv $(echo "${conflicting_packages[*]}")

	# Get the latest systemd portage profile and set it
	systemd_profile=$(profile-config list | grep stable | grep -E "[[:digit:]]/systemd" | xargs | cut -d' ' -f2)
	profile-config set "${systemd_profile}"

	# Install build dependencies
	emerge --newuse $(echo "${build_dependencies[*]}")

	quickpkg --include-unmodified-config=y "*/*"

	# Install needed packages excluding conflicting packages
	ROOT=${ROOTFS_DIR} emerge --exclude "$(echo "${conflicting_packages[*]}")" --newuse -k ${PACKAGES}

	pushd ${ROOTFS_DIR}

	# systemd will need this library
	cp /usr/lib/gcc/x86_64-pc-linux-gnu/*/libgcc_s.so* lib64/

	# Clean up the rootfs. there are things that we don't need
	rm -rf etc/{udev,X11,kernel,runlevels,terminfo,init.d}
	rm -rf var/lib/{gentoo,portage}
	rm -rf var/{db,cache}
	rm -rf usr/share/*
	rm -rf usr/lib/{udev,gconv,kernel}
	rm -rf usr/{include,local}
	rm -rf usr/lib64/gconv
	rm -rf lib/{udev,gentoo}

	# Make sure important directories exist in the rootfs
	ln -s ../run var/run
	mkdir -p proc opt sys dev home root

	popd
}

before_starting_container() {
	gentoo_portage_image="gentoo/portage"

	if [ "${OS_VERSION}" = "latest" ];then
		${container_engine} pull "${gentoo_portage_image}:latest"
		OS_VERSION=$(docker image inspect -f {{.Created}} ${gentoo_portage_image} | cut -dT -f1 | sed 's|-||g')
	else
		${container_engine} pull "${gentoo_portage_image}:${OS_VERSION}"
	fi

	# create portage volume and container
	${container_engine} create -v /usr/portage --name "${gentoo_portage_container}" "${gentoo_portage_image}" /bin/true
}

after_stopping_container() {
	# Get the list of volumes
	volumes=""
	for i in $(seq $(${container_engine} inspect -f "{{len .Mounts}}" "${gentoo_portage_container}")); do
		volumes+="$(${container_engine} inspect -f "{{(index .Mounts $((i-1))).Name}}" "${gentoo_portage_container}") "
	done

	# remove portage container
	${container_engine} rm -f "${gentoo_portage_container}"
	sudo rm -rf "${gentoo_local_portage_dir}"

	# remove portage volumes
	${container_engine} volume rm -f ${volumes}
}
