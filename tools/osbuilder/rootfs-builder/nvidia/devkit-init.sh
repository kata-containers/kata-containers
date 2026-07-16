#!/run/kata-extensions/devkit/bin/busybox sh
# Shared devkit guest runtime library.
#
# Model: the devkit extension (mounted read-only at ${DEVKIT}) is a full, minimal
# Alpine rootfs.  We overlay a writable, exec-enabled tmpfs on top of it and
# chroot in, so apk and every prebaked debug tool run natively against a normal
# root filesystem (musl userland, busybox coreutils, a consistent apk database).
# Sourced by devkit-enter and devkit-apk.

DEVKIT_INIT_VERSION=26

DEVKIT="${DEVKIT:-/run/kata-extensions/devkit}"
WRITABLE="${WRITABLE:-/run/kata-devkit-writable}"
BB="${BB:-${DEVKIT}/bin/busybox}"

UPPER="${WRITABLE}/upper"
WORK="${WRITABLE}/work"
MERGED="${WRITABLE}/merged"

devkit_is_mounted() {
	"${BB}" grep -q "[[:space:]]${1}[[:space:]]" /proc/mounts 2>/dev/null
}

# Mount an exec-enabled tmpfs at ${WRITABLE}: /run is typically noexec, so the
# overlay upper/work (and the fallback rootfs copy) must live on our own tmpfs
# for the prebaked and apt-installed binaries to exec.
devkit_mount_writable() {
	"${BB}" mkdir -p "${WRITABLE}"
	devkit_is_mounted "${WRITABLE}" && return 0
	"${BB}" mount -t tmpfs -o mode=755,size=2G tmpfs "${WRITABLE}"
}

# Build the merged root at ${MERGED}: an overlay of (devkit ro lower + tmpfs rw
# upper), or - if this kernel refuses an overlay mount - a plain copy of the
# (small) rootfs into the tmpfs.
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

# Bind the kernel virtual filesystems into the merged root so apk and the debug
# tools behave.  /dev is rbind'd to bring in pts/shm for interactive shells.
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

# Prepare the chroot.  Idempotent: safe to call from every devkit-* invocation.
devkit_setup_chroot() {
	devkit_mount_writable || return 1
	devkit_mount_root || return 1
	devkit_bind_mounts || return 1
	devkit_seed_resolv_conf
	"${BB}" echo "${DEVKIT_INIT_VERSION}" > "${WRITABLE}/.initialized" 2>/dev/null || true
	return 0
}

# Set up the chroot and exec the given command inside it.
devkit_chroot_exec() {
	devkit_setup_chroot || {
		"${BB}" echo "devkit: failed to set up chroot environment" >&2
		return 1
	}
	exec "${BB}" chroot "${MERGED}" "$@"
}
