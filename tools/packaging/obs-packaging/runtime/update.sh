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

#
# Given the name of a package returns the full package version to be used for
# DEB and RPM dependency constraints as follows, composed of:
# - a version,
# - an optional hash (only for select packages),
# - a release number (only for "deb" packages)
#
pkg_required_ver() {
	local pkg="$1"
	local versionVar="${pkg}_version"
	local hashVar="${pkg}_hash"
	local version=$(echo ${!versionVar})
	local gitHash=

	# Make pkg match the package name on OBS
	pkg="${pkg#kata_}"
	pkg="${pkg//_/-}"
	pkg="${pkg//osbuilder/kata-containers-image}"
	pkg="${pkg//kernel/linux-container}"

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

	local debVer=$(pkg_version "$version" "$release" "$gitHash")
	local rpmVer=$(pkg_version "$version" "" "$gitHash")

	echo  "${debVer}" "${rpmVer}"
}


# Parse arguments
cli "$@"

[ "$VERBOSE" == "true" ] && set -x

declare -a pkgVersions
# Package depedencies
info "Requires:"
declare -A KERNEL_REQUIRED_VERSION
pkgVersions=($(pkg_required_ver "kernel"))
KERNEL_REQUIRED_VERSION["deb"]=${pkgVersions[0]}
KERNEL_REQUIRED_VERSION["rpm"]=${pkgVersions[1]}
info "kata-linux-container ${KERNEL_REQUIRED_VERSION[@]}"

declare -A KATA_IMAGE_REQUIRED_VERSION
pkgVersions=($(pkg_required_ver "kata_osbuilder"))
KATA_IMAGE_REQUIRED_VERSION["deb"]=${pkgVersions[0]}
KATA_IMAGE_REQUIRED_VERSION["rpm"]=${pkgVersions[1]}
info "image ${KATA_IMAGE_REQUIRED_VERSION[@]}"

declare -A KATA_QEMU_VANILLA_REQUIRED_VERSION
pkgVersions=($(pkg_required_ver "qemu_vanilla"))
KATA_QEMU_VANILLA_REQUIRED_VERSION["deb"]=${pkgVersions[0]}
KATA_QEMU_VANILLA_REQUIRED_VERSION["rpm"]=${pkgVersions[1]}
info "qemu-vanilla ${KATA_QEMU_VANILLA_REQUIRED_VERSION[@]}"

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
	"linux_container_version=${KERNEL_REQUIRED_VERSION["rpm"]}"
	"linux_container_version_release=${KERNEL_REQUIRED_VERSION["deb"]}"
	"qemu_vanilla_version=${KATA_QEMU_VANILLA_REQUIRED_VERSION["rpm"]}"
	"qemu_vanilla_version_release=${KATA_QEMU_VANILLA_REQUIRED_VERSION["deb"]}"
)

verify
echo "Verify succeed."
get_git_info
changelog_update $VERSION
generate_files "$SCRIPT_DIR" "${replace_list[@]}"
build_pkg "${PROJECT_REPO}"
