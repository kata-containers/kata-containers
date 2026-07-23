#!/run/kata-extensions/devkit/bin/busybox.static sh
# shellcheck shell=dash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Thin apk wrapper: runs Alpine's apk inside the devkit overlay/chroot, so
# `devkit-apk add htop` installs into the writable overlay at runtime.
DEVKIT=/run/kata-extensions/devkit
# shellcheck source=tools/osbuilder/rootfs-builder/devkit/devkit-init.sh
. "${DEVKIT}/usr/bin/devkit-init"

devkit_chroot_exec /sbin/apk "$@"
