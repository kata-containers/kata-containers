#!/run/kata-extensions/devkit/bin/busybox.static sh
# shellcheck shell=dash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Thin apt wrapper: runs Ubuntu's apt-get inside the devkit overlay/chroot, so
# `devkit-apt update && devkit-apt install -y htop` installs into the writable
# overlay at runtime (an offline base ships no package lists, hence the update).
DEVKIT=/run/kata-extensions/devkit
# shellcheck source=tools/osbuilder/rootfs-builder/devkit/devkit-init.sh
. "${DEVKIT}/usr/bin/devkit-init"

devkit_chroot_exec /usr/bin/apt-get "$@"
