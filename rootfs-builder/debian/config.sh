#
# Copyright (c) 2018 SUSE
#
# SPDX-License-Identifier: Apache-2.0

OS_VERSION=${OS_VERSION:-9.5}

# Set OS_NAME to the desired debian "codename"
OS_NAME=${OS_NAME:-"stretch"}

PACKAGES="systemd iptables init chrony kmod"

# NOTE: Re-using ubuntu rootfs configuration, see 'ubuntu' folder for full content.
source $script_dir/ubuntu/$CONFIG_SH

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=systemd
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
ARCH_EXCLUDE_LIST=()
