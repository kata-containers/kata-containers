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
ln -sfT "${SCRIPT_DIR}/../../kernel/patches/${KR_LTS}.x" "${SCRIPT_DIR}/patches"

KR_PATCHES=$(eval find "${SCRIPT_DIR}/patches" -type f -name "*.patch")

KR_REL=https://www.kernel.org/releases.json
KR_SHA=https://cdn.kernel.org/pub/linux/kernel/v"${KR_SERIES}"/sha256sums.asc

KR_CONFIGS="kata-kernel-configs"

GENERATED_FILES=(kata-linux-container.dsc kata-linux-container.spec _service debian.control ${KR_CONFIGS}.tar.gz)
STATIC_FILES=(debian.dirs debian.rules debian.compat debian.copyright kata-multiarch.sh)
#STATIC_FILES+=($KR_PATCHES)

# Parse arguments
cli "$@"

[ "$VERBOSE" == "true" ] && set -x
PROJECT_REPO=${PROJECT_REPO:-home:${OBS_PROJECT}:${OBS_SUBPROJECT}/linux-container}
RELEASE=$(get_obs_pkg_release "${PROJECT_REPO}")
((RELEASE++))

kernel_sha256=$(curl -L -s -f ${KR_SHA} | awk '/linux-'${VERSION}'.tar.xz/ {print $1}')

# Copy the kernel config files and fragments for all architecture
mkdir -p configs
readonly configs_dir="kernel/configs"
find "${SCRIPT_DIR}/../../${configs_dir}" \( -name "*_kata_kvm_${KR_LTS}.x" -o -name fragments \) -exec tar --transform="s,${configs_dir},${KR_CONFIGS}," -czf ${KR_CONFIGS}.tar.gz {} +

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
generate_files "$SCRIPT_DIR" "${replace_list[@]}"
build_pkg "${PROJECT_REPO}"
