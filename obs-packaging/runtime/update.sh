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
replace_list=()

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

pkg_2_version() {
	local pkg="$1"
	local versionVar="${pkg}_version"
	local hashVar="${pkg}_hash"
	local version=$(echo ${!versionVar})
	local gitHash=

	# Make pkg match the package name on OBS
	pkg="${pkg#kata_}"
	pkg="${pkg//_/-}"
	pkg="${pkg//osbuilder/kata-containers-image}"
	pkg="${pkg//linux/linux-container}"

	if [ -n "${PROJECT_REPO:-}" ]; then
		local proj="${PROJECT_REPO%/runtime}"
	else
		local proj="home:${OBS_PROJECT}:${OBS_SUBPROJECT}"
	fi
	local release="$(get_obs_pkg_release "${proj}/${pkg//_/-}")"

	case "$pkg" in
		linux-container)
			version="${version}.$(cat "${SCRIPT_DIR}/../../kernel/kata_config_version")"
			;;
		qemu-*)
			gitHash=$(echo ${!hashVar}})
			;;
	esac

	pkg_version "$version" "$release" "$gitHash"
}


# Parse arguments
cli "$@"

[ "$VERBOSE" == "true" ] && set -x

# Package depedencies
info "Requires:"
PROXY_REQUIRED_VERSION=$(pkg_2_version "kata_proxy")
info "proxy ${PROXY_REQUIRED_VERSION}"

SHIM_REQUIRED_VERSION=$(pkg_2_version "kata_shim")
info "shim ${SHIM_REQUIRED_VERSION}"

KERNEL_REQUIRED_VERSION=$(pkg_2_version "kata_linux")
info "kata-linux-container ${KERNEL_REQUIRED_VERSION}"

KSM_THROTTLER_REQUIRED_VERSION=$(pkg_2_version "kata_ksm_throttler")
info "ksm-throttler ${KSM_THROTTLER_REQUIRED_VERSION}"

KATA_IMAGE_REQUIRED_VERSION=$(pkg_2_version "kata_osbuilder")
info "image ${KATA_IMAGE_REQUIRED_VERSION}"


KATA_QEMU_VANILLA_REQUIRED_VERSION=$(pkg_2_version "qemu_vanilla")
info "qemu-vanilla ${KATA_QEMU_VANILLA_REQUIRED_VERSION}"

if [ "$arch" == "x86_64" ]; then
	KATA_QEMU_LITE_REQUIRED_VERSION=$(pkg_2_version "qemu_lite")
	info "qemu-lite ${KATA_QEMU_LITE_REQUIRED_VERSION}"
fi

PROJECT_REPO=${PROJECT_REPO:-home:${OBS_PROJECT}:${OBS_SUBPROJECT}/runtime}
RELEASE=$(get_obs_pkg_release "${PROJECT_REPO}")
((RELEASE++))

set_versions "$kata_runtime_hash"

replace_list+=(
	"GO_CHECKSUM=$go_checksum"
	"GO_VERSION=$go_version"
	"GO_ARCH=$GO_ARCH"
	"HASH=$short_hashtag"
	"RELEASE=$RELEASE"
	"VERSION=$VERSION"
	"kata_osbuilder_version=${KATA_IMAGE_REQUIRED_VERSION}"
	"kata_proxy_version=${PROXY_REQUIRED_VERSION}"
	"kata_shim_version=${SHIM_REQUIRED_VERSION}"
	"ksm_throttler_version=${KSM_THROTTLER_REQUIRED_VERSION}"
	"linux_container_version=${KERNEL_REQUIRED_VERSION}"
	"qemu_vanilla_version=${KATA_QEMU_VANILLA_REQUIRED_VERSION}"
)

verify
echo "Verify succeed."
get_git_info
changelog_update $VERSION
generate_files "$SCRIPT_DIR" "${replace_list[@]}"
build_pkg "${PROJECT_REPO}"
