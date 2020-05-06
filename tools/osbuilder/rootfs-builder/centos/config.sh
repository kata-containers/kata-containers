#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME="Centos"

OS_VERSION=${OS_VERSION:-7}

LOG_FILE="/var/log/yum-centos.log"

MIRROR_LIST="http://mirrorlist.centos.org/?release=${OS_VERSION}&arch=${ARCH}&repo=os&container=container"

# Aditional Repos
CENTOS_UPDATES_MIRROR_LIST="http://mirrorlist.centos.org/?release=${OS_VERSION}&arch=${ARCH}&repo=updates&container=container"

CENTOS_EXTRAS_MIRROR_LIST="http://mirrorlist.centos.org/?release=${OS_VERSION}&arch=${ARCH}&repo=extras&container=container"

CENTOS_PLUS_MIRROR_LIST="http://mirrorlist.centos.org/?release=${OS_VERSION}&arch=${ARCH}&repo=centosplus&container=container"

GPG_KEY_URL="https://www.centos.org/keys/RPM-GPG-KEY-CentOS-7"

GPG_KEY_FILE="RPM-GPG-KEY-CentOS-7"

PACKAGES="iptables chrony"

#Optional packages:
# systemd: An init system that will start kata-agent if kata-agent
#          itself is not configured as init process.
[ "$AGENT_INIT" == "no" ] && PACKAGES+=" systemd" || true

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=systemd
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
ARCH_EXCLUDE_LIST=()

[ "$SECCOMP" = "yes" ] && PACKAGES+=" libseccomp" || true
