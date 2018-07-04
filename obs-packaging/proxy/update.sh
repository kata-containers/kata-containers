#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# -*- mode: shell-script; indent-tabs-mode: nil; sh-basic-offset: 4; -*-
# ex: ts=8 sw=4 sts=4 et filetype=sh
#
# Automation script to create specs to build kata-proxy
set -e

source ../versions.txt
source ../scripts/pkglib.sh

SCRIPT_NAME=$0
SCRIPT_DIR=$(dirname "$0")
PKG_NAME="kata-proxy"
VERSION=$kata_proxy_version

GENERATED_FILES=(kata-proxy.spec kata-proxy.dsc debian.control debian.rules _service)
STATIC_FILES=(debian.compat)

# Parse arguments
cli "$@"

[ "$VERBOSE" == "true" ] && set -x
PROJECT_REPO=${PROJECT_REPO:-home:${OBS_PROJECT}:${OBS_SUBPROJECT}/proxy}
RELEASE=$(get_obs_pkg_release "${PROJECT_REPO}")
((RELEASE++))
[ -n "$APIURL" ] && APIURL="-A ${APIURL}"

set_versions $kata_proxy_hash

replace_list=(
"GO_CHECKSUM=$go_checksum"
"GO_VERSION=$go_version"
"GO_ARCH=$GO_ARCH"
"HASH=$short_hashtag"
"RELEASE=$RELEASE"
"VERSION=$VERSION"
)

verify
echo "Verify succeed."
get_git_info
changelog_update $VERSION
generate_files "$SCRIPT_DIR" "${replace_list[@]}"
build_pkg "${PROJECT_REPO}"
