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
[ -z "${DEBUG}" ] || set -o xtrace

set -o errexit
set -o nounset
set -o pipefail

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
PROXY_REQUIRED_VERESION=$(pkg_version "${kata_proxy_version}" "" "")
info "proxy ${PROXY_REQUIRED_VERESION}"

SHIM_REQUIRED_VERSION=$(pkg_version "${kata_shim_version}" "" "")
info "shim ${SHIM_REQUIRED_VERSION}"

KERNEL_CONFIG_VERSION=$(cat "${SCRIPT_DIR}/../../kernel/kata_config_version")
KERNEL_REQUIRED_VERSION=$(pkg_version "${kernel_version}.${KERNEL_CONFIG_VERSION}" "" "")
info "kata-linux-container ${KERNEL_REQUIRED_VERSION}"

KSM_THROTTLER_REQUIRED_VERSION=$(pkg_version "${kata_ksm_throttler_version}" "" "")
info "ksm-throttler ${KSM_THROTTLER_REQUIRED_VERSION}"

KATA_IMAGE_REQUIRED_VERSION=$(pkg_version "${kata_osbuilder_version}" "" "")
info "image ${KATA_IMAGE_REQUIRED_VERSION}"

KATA_QEMU_LITE_REQUIRED_VERSION=$(pkg_version "${qemu_lite_version}" "" "${qemu_lite_hash}")
info "qemu-lite ${KATA_QEMU_LITE_REQUIRED_VERSION}"

KATA_QEMU_VANILLA_REQUIRED_VERSION=$(pkg_version "${qemu_vanilla_version}" "" "${qemu_vanilla_hash}")
info "qemu-vanilla ${KATA_QEMU_VANILLA_REQUIRED_VERSION}"

PROJECT_REPO=${PROJECT_REPO:-home:${OBS_PROJECT}:${OBS_SUBPROJECT}/runtime}
RELEASE=$(get_obs_pkg_release "${PROJECT_REPO}")
((RELEASE++))

set_versions "$kata_runtime_hash"

replace_list=(
	"GO_CHECKSUM=$go_checksum"
	"GO_VERSION=$go_version"
	"GO_ARCH=$GO_ARCH"
	"HASH=$short_hashtag"
	"RELEASE=$RELEASE"
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
