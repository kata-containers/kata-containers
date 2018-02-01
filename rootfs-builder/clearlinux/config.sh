#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

#Use "latest" to always pull the last Clear Linux Release
OS_VERSION=${OS_VERSION:-latest}
PACKAGES="iptables-bin libudev0-shim"
[ "$AGENT_INIT" == "no" ] && PACKAGES+=" systemd" || true
