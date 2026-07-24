#!/run/kata-extensions/devkit/bin/busybox.static sh
# shellcheck shell=dash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Interactive devkit debug shell: the debug_console_shell target, reached via
# ${DEVKIT}/bin/devkit-sh -> this.
#
# NOTE: the bootstrap busybox ash has no builtin/PATH `[`, so use "${BB}" test.
DEVKIT=/run/kata-extensions/devkit
# BB and MERGED are exported by the sourced devkit-init, resolved at runtime.
# shellcheck source=tools/osbuilder/rootfs-builder/devkit/devkit-init.sh
. "${DEVKIT}/usr/bin/devkit-init"

devkit_setup_chroot || {
	"${BB}" echo "devkit: failed to set up chroot environment" >&2
	exit 1
}

# Prefer the prebaked bash, falling back to busybox ash (/bin/sh) if bash is
# missing or not executable in the merged tree.
shell=/bin/bash
"${BB}" test -x "${MERGED}${shell}" || shell=/bin/sh

# The agent execs the shell with no arguments, so default to a login shell; any
# arguments (-i, -c "cmd", ...) pass straight through.
"${BB}" test "$#" -eq 0 && set -- -l

# Enter the chroot in a private UTS namespace whose hostname is "devkit", so the
# prompt makes the devkit overlay obvious without renaming the guest (the change
# is confined to this shell and gone once it exits). Setting the name is
# best-effort; fall back to a plain chroot if unshare is unavailable.
if "${BB}" test -x "${MERGED}/usr/bin/unshare"; then
	# shellcheck disable=SC2016  # $@ must expand in the inner sh, not here
	exec "${BB}" chroot "${MERGED}" /usr/bin/unshare --uts /bin/sh -c \
		'echo devkit > /proc/sys/kernel/hostname 2>/dev/null || true; exec "$@"' \
		devkit-enter "${shell}" "$@"
fi

exec "${BB}" chroot "${MERGED}" "${shell}" "$@"
