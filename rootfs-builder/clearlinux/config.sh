#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME="Clear"

OS_VERSION=${OS_VERSION:-latest}

BASE_URL="https://download.clearlinux.org/current/${ARCH}/os/"

REPO_NAME="clear"

PACKAGES="iptables-bin libudev0-shim"

#Optional packages:
# systemd: An init system that will start kata-agent if kata-agent
#          itself is not configured as init process.
[ "$AGENT_INIT" == "no" ] && PACKAGES+=" systemd" || true
