#!/usr/bin/env bash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0

build_rootfs() {
	local rootfs_dir="$1"

	# mmdebstrap resolves apt dependencies natively (the same tool the base guest
	# rootfs uses), so the devkit needs no chroot or docker-export gymnastics.
	# shellcheck disable=SC2154
	if ! mmdebstrap --mode auto --arch "${DEB_ARCH}" --variant required \
			--components="${REPO_COMPONENTS}" \
			--include "${PACKAGES}${EXTRA_PKGS:+ ${EXTRA_PKGS}}" \
			"${OS_VERSION}" "${rootfs_dir}" "${REPO_URL}"; then
		die "mmdebstrap failed for the devkit rootfs"
	fi

	# Expose busybox-static at the path devkit-init bootstraps from on the
	# shell-less guest base.
	local bb_src="" cand
	for cand in usr/bin/busybox bin/busybox usr/sbin/busybox sbin/busybox; do
		[[ -f "${rootfs_dir}/${cand}" ]] && { bb_src="${cand}"; break; }
	done
	[[ -n "${bb_src}" ]] || die "busybox-static not found in the devkit rootfs"
	cp -a "${rootfs_dir}/${bb_src}" "${rootfs_dir}/usr/bin/busybox.static"

	# Install the guest-side helper scripts and expose the debug console entry as
	# devkit-sh (a sibling symlink to devkit-enter; Ubuntu's /bin is a symlink to
	# /usr/bin, so /run/kata-extensions/devkit/bin/devkit-sh resolves here).
	local script dest
	# CONFIG_DIR is exported by rootfs.sh (the sourcing build system).
	# shellcheck disable=SC2154
	for script in devkit-init.sh devkit-enter.sh devkit-add-nvidia-repos.sh; do
		[[ -f "${CONFIG_DIR}/${script}" ]] || die "missing ${CONFIG_DIR}/${script}"
		dest="${script%.sh}"
		install -D -m0755 "${CONFIG_DIR}/${script}" "${rootfs_dir}/usr/bin/${dest}"
	done
	ln -sf devkit-enter "${rootfs_dir}/usr/bin/devkit-sh"

	# apt in the debug overlay runs as root: this is a single-user chroot and the
	# _apt sandbox user cannot create its temp files (apt-key config, partials) on
	# the tmpfs overlay, which otherwise breaks `apt-get update`. Keep /tmp sticky
	# for the same reason.
	install -d -m0755 "${rootfs_dir}/etc/apt/apt.conf.d"
	echo 'APT::Sandbox::User "root";' > "${rootfs_dir}/etc/apt/apt.conf.d/99-devkit.conf"
	chmod 0644 "${rootfs_dir}/etc/apt/apt.conf.d/99-devkit.conf"
	chmod 1777 "${rootfs_dir}/tmp" 2>/dev/null || true

	# Trim what a debug rootfs does not need (man/doc/info, apt lists/cache) and
	# the seeded resolver (devkit-init re-seeds one at runtime).
	rm -rf \
		"${rootfs_dir}/usr/share/man"/* \
		"${rootfs_dir}/usr/share/doc"/* \
		"${rootfs_dir}/usr/share/info"/* \
		"${rootfs_dir}/var/lib/apt/lists"/* \
		"${rootfs_dir}/var/cache/apt"/* \
		"${rootfs_dir}/etc/resolv.conf" \
		2>/dev/null || true

	# The mount point must be searchable once the extension is mounted read-only.
	chmod 0755 "${rootfs_dir}"
}
