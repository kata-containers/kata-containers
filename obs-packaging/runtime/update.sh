#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# -*- mode: shell-script; indent-tabs-mode: nil; sh-basic-offset: 4; -*-
# ex: ts=8 sw=4 sts=4 et filetype=sh
#
# Automation script to create specs to build kata-runtime
# Default: Build is the one specified in file configure.ac
# located at the root of the repository.
set -e

source ../versions.txt
source ../scripts/pkglib.sh

SCRIPT_NAME=$0
SCRIPT_DIR=$(dirname "$0")

# Package information
# Used by pkglib.sh
export PKG_NAME="kata-runtime"
VERSION=$kata_runtime_version

# Used by pkglib
export GENERATED_FILES=(kata-runtime.spec kata-runtime.dsc debian.control debian.rules _service)
# Used by pkglib
export STATIC_FILES=(debian.compat)

#cli flags
LOCAL_BUILD=false
OBS_PUSH=false
VERBOSE=false

# Parse arguments
cli "$@"

[ "$VERBOSE" == "true" ] && set -x

# Package depedencies
info "requires:"
PROXY_RELEASE=$(get_obs_pkg_release "home:${OBS_PROJECT}:${OBS_SUBPROJECT}/proxy")
PROXY_REQUIRED_VERESION=$(pkg_version "${kata_proxy_version}" "${PROXY_RELEASE}" "${kata_proxy_hash}")
info "proxy ${PROXY_REQUIRED_VERESION}"

SHIM_RELEASE=$(get_obs_pkg_release "home:${OBS_PROJECT}:${OBS_SUBPROJECT}/shim")
SHIM_REQUIRED_VERSION=$(pkg_version "${kata_shim_version}" "${SHIM_RELEASE}" "${kata_shim_hash}")
info "shim ${SHIM_REQUIRED_VERSION}"

KERNEL_RELEASE=$(get_obs_pkg_release "home:${OBS_PROJECT}:${OBS_SUBPROJECT}/linux-container")
KERNEL_CONFIG_VERSION=$(cat "${SCRIPT_DIR}/../kernel/kata_config_version")
KERNEL_REQUIRED_VERSION=$(pkg_version "${kernel_version}.${KERNEL_CONFIG_VERSION}" "${KERNEL_RELEASE}")
info "kata-linux-container ${KERNEL_REQUIRED_VERSION}"

KSM_THROTTLER_RELEASE=$(get_obs_pkg_release "home:${OBS_PROJECT}:${OBS_SUBPROJECT}/ksm-throttler")
KSM_THROTTLER_REQUIRED_VERSION=$(pkg_version "${ksm_throttler_version}" "${KSM_THROTTLER_RELEASE}" "${ksm_throttler_hash}")
info "ksm-throttler ${KSM_THROTTLER_REQUIRED_VERSION}"

KATA_CONTAINERS_IMAGE_RELEASE=$(get_obs_pkg_release "home:${OBS_PROJECT}:${OBS_SUBPROJECT}/kata-containers-image")
KATA_IMAGE_REQUIRED_VERSION=$(pkg_version "${kata_osbuilder_version}" "${KATA_CONTAINERS_IMAGE_RELEASE}")
info "image ${KATA_IMAGE_REQUIRED_VERSION}"

KATA_CONTAINERS_QEMU_LITE_RELEASE=$(get_obs_pkg_release "home:${OBS_PROJECT}:${OBS_SUBPROJECT}/qemu-lite")
KATA_QEMU_LITE_REQUIRED_VERSION=$(pkg_version "${qemu_lite_version}" "${KATA_CONTAINERS_QEMU_LITE_RELEASE}")
info "image ${KATA_QEMU_LITE_REQUIRED_VERSION}"

KATA_CONTAINERS_QEMU_VANILLA_RELEASE=$(get_obs_pkg_release "home:${OBS_PROJECT}:${OBS_SUBPROJECT}/qemu-vanilla")
KATA_QEMU_VANILLA_REQUIRED_VERSION=$(pkg_version "${qemu_vanilla_version}" "${KATA_CONTAINERS_QEMU_VANILLA_RELEASE}")
info "image ${KATA_QEMU_VANILLA_REQUIRED_VERSION}"

PROJECT_REPO=${PROJECT_REPO:-home:${OBS_PROJECT}:${OBS_SUBPROJECT}/runtime}
RELEASE=$(get_obs_pkg_release "${PROJECT_REPO}")
((RELEASE++))

[ -n "$APIURL" ] && APIURL="-A ${APIURL}"

set_versions "$kata_runtime_hash"

replace_list=(
"GO_CHECKSUM=$go_checksum"
"GO_VERSION=$go_version"
"GO_ARCH=$GO_ARCH"
"HASH=$short_hashtag"
"RELEASE=$RELEASE"
"REVISION=$VERSION"
"VERSION=$VERSION"
"kata_osbuilder_version=${KATA_IMAGE_REQUIRED_VERSION}"
"kata_proxy_version=${PROXY_REQUIRED_VERESION}"
"kata_shim_version=${SHIM_REQUIRED_VERSION}"
"ksm_throttler_version=${KSM_THROTTLER_REQUIRED_VERSION}"
"linux_container_version=${KERNEL_REQUIRED_VERSION}"
"qemu_lite_version=${KATA_QEMU_LITE_REQUIRED_VERSION}"
"qemu_vanilla_version=${KATA_QEMU_VANILLA_REQUIRED_VERSION}"
)


verify
echo "Verify succeed."
get_git_info
changelog_update $VERSION
generate_files "$SCRIPT_DIR" "${replace_list[@]}"
build_pkg "${PROJECT_REPO}"

