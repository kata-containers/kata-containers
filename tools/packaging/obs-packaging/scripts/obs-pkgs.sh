#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

#NOTES:
# - update qemu and the kernel first, they take longer to build
# - runtime is always built at the end, as it depends on all the other listed
# packages, and we need to get the full version of all those.

typeset -a OBS_PKGS_PROJECTS

OBS_PKGS_PROJECTS+=(
	qemu-vanilla
	linux-container
	kata-containers-image
	runtime
)
