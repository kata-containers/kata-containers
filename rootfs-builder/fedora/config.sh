#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME="Fedora"

OS_VERSION=${OS_VERSION:-28}

MIRROR_LIST="https://mirrors.fedoraproject.org/metalink?repo=fedora-${OS_VERSION}&arch=\$basearch"

PACKAGES="iptables"

#Optional packages:
# systemd: An init system that will start kata-agent if kata-agent
#          itself is not configured as init process.
[ "$AGENT_INIT" == "no" ] && PACKAGES+=" systemd" || true

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=systemd
ARCH_EXCLUDE_LIST=()
