#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# -*- mode: shell-script; indent-tabs-mode: nil; sh-basic-offset: 4; -*-
# ex: ts=8 sw=4 sts=4 et filetype=sh
#
# Automation script to create specs to build ksm-throttler.
# Default: Build is the one specified in file configure.ac
# located at the root of the repository.
set -e

source ../versions.txt
source ../scripts/pkglib.sh

SCRIPT_NAME=$0
SCRIPT_DIR=$(dirname $0)
PKG_NAME="kata-ksm-throttler"
VERSION=$ksm_throttler_version
HASH=$ksm_throttler_hash

GENERATED_FILES=(_service kata-ksm-throttler.spec kata-ksm-throttler.dsc debian.control debian.rules)
STATIC_FILES=(debian.compat)

# Parse arguments
cli "$@"

[ "$VERBOSE" == "true" ] && set -x
PROJECT_REPO=${PROJECT_REPO:-home:${OBS_PROJECT}:${OBS_SUBPROJECT}/ksm-throttler}
RELEASE=$(get_obs_pkg_release "${PROJECT_REPO}")
((RELEASE++))
[ -n "$APIURL" ] && APIURL="-A ${APIURL}"


set_versions "$ksm_throttler_hash"

replace_list=(
"GO_CHECKSUM=$go_checksum"
"GO_VERSION=$go_version"
"GO_ARCH=$GO_ARCH"
"HASH=${HASH:0:7}"
"RELEASE=$RELEASE"
"REVISION=$HASH"
"VERSION=$VERSION"
)

verify
echo "Verify succeed."
get_git_info
changelog_update $VERSION
generate_files "$SCRIPT_DIR" "${replace_list[@]}"
build_pkg "${PROJECT_REPO}"
