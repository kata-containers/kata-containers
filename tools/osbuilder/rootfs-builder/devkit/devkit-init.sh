#!/run/kata-extensions/devkit/bin/busybox.static sh
# shellcheck shell=dash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Shared devkit guest runtime library, sourced by devkit-enter.
#
# Nothing here runs at boot: devkit-enter is the debug console's shell, so this
# is driven only when someone attaches to the console, long after every
# extension has been mounted. The order the extensions were mounted in is
# therefore irrelevant, and a sandbox nobody attaches to pays nothing.
#
# The devkit extension (read-only at ${DEVKIT}) is a full minimal Ubuntu rootfs.
# We overlay a writable, exec-enabled tmpfs on it and chroot in, so apt and the
# prebaked tools run natively against a normal root filesystem.
#
# BB is a statically-linked busybox (busybox-static, /bin/busybox.static), the
# ONLY binary we can exec on the shell-less guest base before the glibc dynamic
# loader is reachable there. It is installed at a dedicated path so it never
# clobbers the rootfs's own /bin/busybox or GNU coreutils.

DEVKIT_INIT_VERSION=1

EXTENSIONS_ROOT="${EXTENSIONS_ROOT:-/run/kata-extensions}"
DEVKIT="${DEVKIT:-${EXTENSIONS_ROOT}/devkit}"
WRITABLE="${WRITABLE:-/run/kata-devkit-writable}"
BB="${BB:-${DEVKIT}/bin/busybox.static}"

UPPER="${WRITABLE}/upper"
WORK="${WRITABLE}/work"
MERGED="${WRITABLE}/merged"

# The agent execs the debug console shell with the environment PID 1 happens to
# have, which on a minimal guest may carry no PATH at all, so the chroot starts
# from an explicit one rather than whatever the shell falls back to.
DEVKIT_BASE_PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

# Tool directories an extension may ship, relative to its mount root. The GPU
# extension keeps its binaries in bin/sbin, the CoCo one under usr/local; list
# both layouts so no extension needs to know about this file.
DEVKIT_EXTENSION_BIN_DIRS="bin sbin usr/bin usr/sbin usr/local/bin usr/local/sbin"

devkit_is_mounted() {
	"${BB}" grep -q "[[:space:]]${1}[[:space:]]" /proc/mounts 2>/dev/null
}

# Colon-separated tool directories of the extensions mounted in this sandbox,
# named as the *guest* root sees them (/run/kata-extensions/<name>/...).
#
# Those are the paths that matter for the common debug move of reaching into the
# guest from the devkit shell: `chroot /real_root <tool>` resolves <tool> with
# execvp() *after* switching root, i.e. against the guest's filesystem, using the
# PATH it inherits from us. Without these entries an extension's tools are only
# reachable by full path ("chroot: failed to run command 'nvidia-smi'").
#
# The devkit extension itself is skipped: it is already the chroot's own root,
# and its dynamically linked binaries cannot run on the shell-less guest base.
devkit_extension_path() {
	local path="" ext sub dir
	for ext in "${EXTENSIONS_ROOT}"/*; do
		"${BB}" test -d "${ext}" || continue
		"${BB}" test "${ext}" = "${DEVKIT}" && continue

		# An extension name is a mount point the runtime created from the
		# hypervisor config, but PATH has no quoting: anything that could
		# split an entry has no business in it.
		case "${ext##*/}" in
			*[!A-Za-z0-9._-]*) continue ;;
		esac

		for sub in ${DEVKIT_EXTENSION_BIN_DIRS}; do
			dir="${ext}/${sub}"
			"${BB}" test -d "${dir}" || continue
			# A symlinked directory (the GPU extension's usr/bin ->
			# ../bin) is just a second name for one already listed.
			"${BB}" test -L "${dir}" && continue
			path="${path:+${path}:}${dir}"
		done
	done
	"${BB}" echo "${path}"
}

# Seed the PATH the chroot (and anything it execs, including `chroot /real_root`)
# runs with. Ubuntu's /etc/profile only sources /etc/profile.d, it does not reset
# PATH, so a login shell keeps what we export here.
devkit_export_path() {
	local extension_path
	extension_path="$(devkit_extension_path)"
	PATH="${DEVKIT_BASE_PATH}${extension_path:+:${extension_path}}"
	export PATH
}

# Mount an exec-enabled tmpfs at ${WRITABLE}: /run is typically noexec, so the
# overlay upper/work (and the fallback rootfs copy) must live on our own tmpfs
# for the prebaked and apt-installed binaries to exec.
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

# Bind the kernel virtual filesystems so apt and the debug tools behave; /dev is
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

# Give apt/curl working DNS by importing the guest resolver config.
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

# Idempotent: safe to call from every devkit-* invocation. Also exports the PATH
# the caller then hands to the chroot, so mounting an extension is all it takes
# for its tools to be reachable by name.
devkit_setup_chroot() {
	devkit_mount_writable || return 1
	devkit_mount_root || return 1
	devkit_bind_mounts || return 1
	devkit_seed_resolv_conf
	devkit_link_real_root
	devkit_export_path
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
