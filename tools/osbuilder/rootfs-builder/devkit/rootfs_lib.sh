#!/usr/bin/env bash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0

build_rootfs() {
	# Everything below writes to paths derived from this one, so an empty value
	# would aim the whole build (including its rm -rf) at the build machine.
	local rootfs_dir="${1:?rootfs_dir is required}"
	[[ "${rootfs_dir}" != "/" ]] || die "refusing to build the devkit rootfs at /"

	# mmdebstrap resolves apt dependencies natively (the same tool the base guest
	# rootfs uses), so the devkit needs no chroot or docker-export gymnastics.
	#
	# For debootstrap compatibility it copies the build machine's
	# /etc/{resolv.conf,hostname} into the target, so drop them again once apt is
	# done (upstream's documented recipe): the devkit seeds its resolver from the
	# guest at runtime (devkit-init), and a released artefact must carry neither
	# the build machine's DNS servers nor its hostname.
	# shellcheck disable=SC2016,SC2154
	if ! mmdebstrap --mode auto --arch "${DEB_ARCH}" --variant required \
			--components="${REPO_COMPONENTS}" \
			--include "${PACKAGES[*]}${EXTRA_PKGS:+ ${EXTRA_PKGS}}" \
			--customize-hook='rm -f "$1/etc/resolv.conf" "$1/etc/hostname"' \
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

	# devkit-add-nvidia-repos runs inside the guest, where versions.yaml is not
	# available, so resolve the pinned CUDA repository here and bake it in. Arches
	# with no entry get the placeholders substituted away, which makes the helper
	# refuse to run rather than reach for an unpinned URL.
	local cuda_repo_url cuda_repo_pkg
	# ARCH is exported by rootfs.sh.
	# shellcheck disable=SC2154
	cuda_repo_url=$(get_package_version_from_kata_yaml "externals.nvidia.cuda.repo.${ARCH}.url")
	cuda_repo_pkg=$(get_package_version_from_kata_yaml "externals.nvidia.cuda.repo.${ARCH}.pkg")
	[[ -n "${cuda_repo_url}" ]] \
		|| info "no externals.nvidia.cuda.repo entry for ${ARCH}: devkit-add-nvidia-repos will be inert"
	sed -i \
		-e "s|@CUDA_REPO_URL@|${cuda_repo_url}|g" \
		-e "s|@CUDA_REPO_PKG@|${cuda_repo_pkg}|g" \
		"${rootfs_dir}/usr/bin/devkit-add-nvidia-repos"

	# apt in the debug overlay runs as root: this is a single-user chroot and the
	# _apt sandbox user cannot create its temp files (apt-key config, partials) on
	# the tmpfs overlay, which otherwise breaks `apt-get update`. Keep /tmp sticky
	# for the same reason.
	install -d -m0755 "${rootfs_dir}/etc/apt/apt.conf.d"
	echo 'APT::Sandbox::User "root";' > "${rootfs_dir}/etc/apt/apt.conf.d/99-devkit.conf"
	chmod 0644 "${rootfs_dir}/etc/apt/apt.conf.d/99-devkit.conf"
	chmod 1777 "${rootfs_dir}/tmp" 2>/dev/null || true

	# Trim what a debug rootfs does not need (man/doc/info, apt lists/cache). The
	# resolver is already gone, dropped by the mmdebstrap hook above.
	rm -rf \
		"${rootfs_dir}/usr/share/man"/* \
		"${rootfs_dir}/usr/share/doc"/* \
		"${rootfs_dir}/usr/share/info"/* \
		"${rootfs_dir}/var/lib/apt/lists"/* \
		"${rootfs_dir}/var/cache/apt"/* \
		2>/dev/null || true

	# The mount point must be searchable once the extension is mounted read-only.
	chmod 0755 "${rootfs_dir}"
}
