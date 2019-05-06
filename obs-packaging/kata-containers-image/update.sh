#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# -*- mode: shell-script; indent-tabs-mode: nil; sh-basic-offset: 4; -*-
# ex: ts=8 sw=4 sts=4 et filetype=sh

# Automation script to create specs to build kata-containers-image
# Default image to build is the one specified in file versions.txt
# located at the root of the repository.

[ -z "${DEBUG}" ] || set -o xtrace

set -o errexit
set -o nounset
set -o pipefail

source ../versions.txt
source ../scripts/pkglib.sh

SCRIPT_NAME=$0
SCRIPT_DIR=$(dirname $0)
PKG_NAME="kata-containers-image"
VERSION=$kata_osbuilder_version

GENERATED_FILES=(kata-containers-image.spec kata-containers-image.dsc debian.rules debian.control)
STATIC_FILES=(LICENSE debian.compat debian.dirs kata-containers.tar.gz)

# Parse arguments
cli "$@"

[ "$VERBOSE" == "true" ] && set -x
PROJECT_REPO=${PROJECT_REPO:-home:${OBS_PROJECT}:${OBS_SUBPROJECT}/kata-containers-image}
RELEASE=$(get_obs_pkg_release "${PROJECT_REPO}")
((RELEASE++))

function check_image() {
	[ ! -f "${SCRIPT_DIR}/kata-containers.tar.gz" ] && die "No kata-containers.tar.gz found!\nUse the build_image.sh script" || echo "Image: OK"
}

replace_list=(
	"VERSION=$VERSION"
	"RELEASE=$RELEASE"
)

verify
rm -rf kata-containers.tar.gz
image_tarball=$(find . -name 'kata-containers-'"${VERSION/\~/-}"'-'"${kata_agent_hash:0:${short_commit_length}}"'*-*.tar.gz')
[ -f "${image_tarball}" ] || die "image not found"
cp "${image_tarball}" kata-containers.tar.gz

check_image
echo "Verify succeed."
get_git_info
#TODO delete me: used by changelog_update
hash_tag="nocommit"
changelog_update $VERSION
generate_files "$SCRIPT_DIR" "${replace_list[@]}"
build_pkg "${PROJECT_REPO}"
