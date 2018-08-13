#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# -*- mode: shell-script; indent-tabs-mode: nil; sh-basic-offset: 4; -*-
# ex: ts=8 sw=4 sts=4 et filetype=sh

# Automation script to create specs to build Kata containers kernel
[ -z "${DEBUG}" ] || set -o xtrace

set -o errexit
set -o nounset
set -o pipefail

source ../versions.txt
source ../scripts/pkglib.sh

SCRIPT_NAME=$0
SCRIPT_DIR=$(dirname $0)

PKG_NAME="kata-linux-container"
VERSION=$kernel_version
KATA_CONFIG_VERSION=$(cat "${SCRIPT_DIR}/../../kernel/kata_config_version")

KR_SERIES="$(echo $VERSION | cut -d "." -f 1).x"
KR_LTS=$(echo $VERSION | cut -d "." -f 1,2)
KR_PATCHES=$(eval find "${SCRIPT_DIR}/../../kernel/patches" -type f -name "*.patch")

KR_REL=https://www.kernel.org/releases.json
KR_SHA=https://cdn.kernel.org/pub/linux/kernel/v"${KR_SERIES}"/sha256sums.asc

GENERATED_FILES=(kata-linux-container.dsc kata-linux-container.spec _service config debian.control)
STATIC_FILES=(debian.dirs debian.rules debian.compat debian.copyright)
#STATIC_FILES+=($KR_PATCHES)

# Parse arguments
cli "$@"

[ "$VERBOSE" == "true" ] && set -x
PROJECT_REPO=${PROJECT_REPO:-home:${OBS_PROJECT}:${OBS_SUBPROJECT}/linux-container}
RELEASE=$(get_obs_pkg_release "${PROJECT_REPO}")
((RELEASE++))

kernel_sha256=$(curl -L -s -f ${KR_SHA} | awk '/linux-'${VERSION}'.tar.xz/ {print $1}')

# Generate the kernel config file
KERNEL_ARCH=$(go get github.com/kata-containers/tests && $GOPATH/src/github.com/kata-containers/tests/.ci/kata-arch.sh --kernel)
cp "${SCRIPT_DIR}/../../kernel/configs/${KERNEL_ARCH}_kata_kvm_${KR_LTS}.x" config

replace_list=(
"VERSION=${VERSION}"
"CONFIG_VERSION=${KATA_CONFIG_VERSION}"
"RELEASE=$RELEASE"
"KERNEL_SHA256=$kernel_sha256"
)

verify
echo "Verify succeed."
get_git_info
#TODO delete me: used by changelog_update
hash_tag="nocommit"
changelog_update "${VERSION}-${KATA_CONFIG_VERSION}"
ln -sfT "${SCRIPT_DIR}/../../kernel/patches" "${SCRIPT_DIR}/patches"
generate_files "$SCRIPT_DIR" "${replace_list[@]}"
build_pkg "${PROJECT_REPO}"
