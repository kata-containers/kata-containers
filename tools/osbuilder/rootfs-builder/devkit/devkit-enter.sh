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

exec "${BB}" chroot "${MERGED}" "${shell}" "$@"
