#!/bin/bash
#
# Copyright (c) 2019 IBM
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

OSBUILDER_DISTRO="ubuntu"
AGENT_INIT="yes"

#packaged kata agent haven't been supported in any mainstream distribution
get_packaged_agent_version() {
        version=""
        echo "$version"
}

#packaged kata image haven't been supported in any mainstream distribution
install_packaged_image() {
        info "installing packaged kata-image not supported in s390x"
	return 1
}
