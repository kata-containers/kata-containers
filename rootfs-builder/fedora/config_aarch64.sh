#
# Copyright (c) 2019 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

# image busybox will fail on fedora 30 rootfs image
# see https://github.com/kata-containers/osbuilder/issues/334 for detailed info
OS_VERSION="29"

MIRROR_LIST="https://mirrors.fedoraproject.org/metalink?repo=fedora-${OS_VERSION}&arch=\$basearch"
