#!/run/kata-extensions/devkit/bin/busybox sh
# Interactive devkit debug shell.  Overlays a writable tmpfs on the read-only
# Alpine devkit rootfs, chroots into the merged tree and execs a shell.  This is
# the debug_console_shell target, reached via ${DEVKIT}/bin/sh -> devkit-enter.
#
# NOTE: the bootstrap busybox ash has no builtin/PATH `[`, so use "${BB}" test.
DEVKIT=/run/kata-extensions/devkit
. "${DEVKIT}/usr/bin/devkit-init"

devkit_setup_chroot || {
	"${BB}" echo "devkit: failed to set up chroot environment" >&2
	exit 1
}

# Prefer the prebaked bash for an interactive session, but fall back to Alpine's
# busybox ash (/bin/sh) if bash is missing or not executable in the merged tree.
shell=/bin/bash
"${BB}" test -x "${MERGED}${shell}" || shell=/bin/sh

# The agent execs the debug shell with no arguments; default to a login shell.
# Any arguments (e.g. -i, or -c "cmd") are passed straight through.
"${BB}" test "$#" -eq 0 && set -- -l

exec "${BB}" chroot "${MERGED}" "${shell}" "$@"
