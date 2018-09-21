#
# Copyright (c) 2018 SUSE
#
# SPDX-License-Identifier: Apache-2.0

OS_VERSION=${OS_VERSION:-9.5}

# Set OS_NAME to the desired debian "codename"
OS_NAME=${OS_NAME:-"stretch"}

# NOTE: Re-using ubuntu rootfs configuration, see 'ubuntu' folder for full content.
source $script_dir/ubuntu/$CONFIG_SH
