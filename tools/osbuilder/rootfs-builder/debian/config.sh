# Copyright (c) 2018 SUSE
# Copyright (c) 2023 Sony Group Corporation
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME=${OS_NAME:-"debian"}
# This should be Debian's code name, e.g. "bookworm" for Debian 12.x
OS_VERSION=${OS_VERSION:-bookworm}

# NOTE: Re-using ubuntu rootfs configuration, see 'ubuntu' folder for full content.
source $script_dir/ubuntu/$CONFIG_SH

REPO_URL="http://deb.debian.org/debian"
KEYRING="debian-archive-keyring"
