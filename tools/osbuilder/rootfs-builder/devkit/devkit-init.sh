#!/run/kata-extensions/devkit/bin/busybox.static sh
# shellcheck shell=dash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Shared devkit guest runtime library, sourced by devkit-enter and devkit-apk.
#
# The devkit extension (read-only at ${DEVKIT}) is a full minimal Alpine rootfs.
# We overlay a writable, exec-enabled tmpfs on it and chroot in, so apk and the
# prebaked tools run natively against a normal root filesystem.
#
# BB is Alpine's statically-linked busybox (busybox-static, /bin/busybox.static),
# the ONLY binary we can exec on the shell-less guest base before the musl loader
# exists. We deliberately do NOT reuse /bin/busybox: that is Alpine's dynamic
# busybox providing the chroot's coreutils applets, and clobbering it breaks
# ls/cat/ps and apk's busybox trigger.

DEVKIT_INIT_VERSION=1

DEVKIT="${DEVKIT:-/run/kata-extensions/devkit}"
WRITABLE="${WRITABLE:-/run/kata-devkit-writable}"
BB="${BB:-${DEVKIT}/bin/busybox.static}"

UPPER="${WRITABLE}/upper"
WORK="${WRITABLE}/work"
MERGED="${WRITABLE}/merged"

devkit_is_mounted() {
	"${BB}" grep -q "[[:space:]]${1}[[:space:]]" /proc/mounts 2>/dev/null
}

# Mount an exec-enabled tmpfs at ${WRITABLE}: /run is typically noexec, so the
# overlay upper/work (and the fallback rootfs copy) must live on our own tmpfs
# for the prebaked and apk-installed binaries to exec.
devkit_mount_writable() {
	"${BB}" mkdir -p "${WRITABLE}"
	devkit_is_mounted "${WRITABLE}" && return 0
	"${BB}" mount -t tmpfs -o mode=755,size=2G tmpfs "${WRITABLE}"
}

# Overlay (devkit ro lower + tmpfs rw upper), falling back to a plain copy of the
# rootfs into the tmpfs if this kernel refuses an overlay mount.
devkit_mount_root() {
	"${BB}" mkdir -p "${UPPER}" "${WORK}" "${MERGED}"
	devkit_is_mounted "${MERGED}" && return 0
	"${BB}" test -f "${WRITABLE}/.copied" && return 0

	local ovl_err
	if ovl_err="$("${BB}" mount -t overlay overlay "${MERGED}" \
		-o "lowerdir=${DEVKIT},upperdir=${UPPER},workdir=${WORK}" 2>&1)" \
		&& devkit_is_mounted "${MERGED}"; then
		return 0
	fi

	# Fallback: overlay unavailable on this kernel - copy the rootfs into tmpfs.
	"${BB}" echo "devkit: overlay mount unavailable (${ovl_err:-unknown}), copying rootfs into tmpfs" >&2
	"${BB}" cp -a "${DEVKIT}/." "${MERGED}/"
	"${BB}" touch "${WRITABLE}/.copied"
}

# Bind the kernel virtual filesystems so apk and the debug tools behave; /dev is
# rbind'd to bring in pts/shm for interactive shells.
devkit_bind_mounts() {
	"${BB}" mkdir -p "${MERGED}/proc" "${MERGED}/sys" "${MERGED}/dev"

	devkit_is_mounted "${MERGED}/proc" || "${BB}" mount -t proc proc "${MERGED}/proc"
	devkit_is_mounted "${MERGED}/sys"  || "${BB}" mount -t sysfs sysfs "${MERGED}/sys"

	if ! devkit_is_mounted "${MERGED}/dev"; then
		if ! "${BB}" mount -o rbind /dev "${MERGED}/dev" 2>/dev/null; then
			"${BB}" mount -t devtmpfs devtmpfs "${MERGED}/dev" 2>/dev/null \
				|| "${BB}" mount -t tmpfs tmpfs "${MERGED}/dev"
			"${BB}" mkdir -p "${MERGED}/dev/pts"
			"${BB}" mount -t devpts devpts "${MERGED}/dev/pts" 2>/dev/null || true
		fi
	fi
}

# Give apk/curl working DNS by importing the guest resolver config.
devkit_seed_resolv_conf() {
	"${BB}" test -e /etc/resolv.conf || return 0
	"${BB}" mkdir -p "${MERGED}/etc"
	"${BB}" cp -L /etc/resolv.conf "${MERGED}/etc/resolv.conf" 2>/dev/null || true
}

# Expose the guest's real root as /real_root so container rootfses and other
# guest state are reachable from the debug shell (e.g.
# /real_root/run/kata-containers/<id>/rootfs).
#
# A symlink to /proc/1/root, not a bind mount: proc is mounted in the chroot, so
# the kernel resolves it to PID 1's root regardless of the chroot, with no
# bind-mount recursion or teardown ordering to worry about.
devkit_link_real_root() {
	"${BB}" test -e "${MERGED}/real_root" && return 0
	"${BB}" ln -s /proc/1/root "${MERGED}/real_root" 2>/dev/null || true
}

# Idempotent: safe to call from every devkit-* invocation.
devkit_setup_chroot() {
	devkit_mount_writable || return 1
	devkit_mount_root || return 1
	devkit_bind_mounts || return 1
	devkit_seed_resolv_conf
	devkit_link_real_root
	"${BB}" echo "${DEVKIT_INIT_VERSION}" > "${WRITABLE}/.initialized" 2>/dev/null || true
	return 0
}

devkit_chroot_exec() {
	devkit_setup_chroot || {
		"${BB}" echo "devkit: failed to set up chroot environment" >&2
		return 1
	}
	exec "${BB}" chroot "${MERGED}" "$@"
}
