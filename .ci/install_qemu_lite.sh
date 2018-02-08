#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

arch=$(arch)

if [ "$#" -ne 3 ]; then
    echo "Usage: $0 <CLEAR_RELEASE> <QEMU_LITE_VERSION> <DISTRO>"
    echo "       Install the QEMU_LITE_VERSION from clear CLEAR_RELEASE."
    exit 1
fi

clear_release="$1"
qemu_lite_version="$2"
distro="$3"
qemu_lite_bin="qemu-lite-bin-${qemu_lite_version}.${arch}.rpm"
qemu_lite_data="qemu-lite-data-${qemu_lite_version}.${arch}.rpm"

echo -e "Install qemu-lite ${qemu_lite_version}"

# download packages
curl -LO "https://download.clearlinux.org/releases/${clear_release}/clear/${arch}/os/Packages/${qemu_lite_bin}"
curl -LO "https://download.clearlinux.org/releases/${clear_release}/clear/${arch}/os/Packages/${qemu_lite_data}"

# install packages
if [ "$distro" == "ubuntu" ];  then
	sudo alien -i "./${qemu_lite_bin}"
	sudo alien -i "./${qemu_lite_data}"
fi

# cleanup
rm -f "./${qemu_lite_bin}"
rm -f "./${qemu_lite_data}"
