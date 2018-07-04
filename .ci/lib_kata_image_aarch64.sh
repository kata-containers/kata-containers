#!/bin/bash
#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -e

IMAGE_TYPE="assets.image.meta.image-type-aarch64"

#packaged kata agent haven't been supported in any mainstream distribution
get_packaged_agent_version() {
	version=""
	echo "$version"
}

#packaged kata image haven't been supported in any mainstream distribution
install_packaged_image() {
	info "installing packaged kata-image not supported in aarch64"
}

