#!/usr/bin/env bash
#
# Copyright (c) 2022 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
runtimedir=$cidir/../src/runtime

build_working_packages() {
	# working packages:
	device_api=$runtimedir/virtcontainers/device/api
	device_config=$runtimedir/virtcontainers/device/config
	device_drivers=$runtimedir/virtcontainers/device/drivers
	device_manager=$runtimedir/virtcontainers/device/manager
	rc_pkg_dir=$runtimedir/pkg/resourcecontrol/
	utils_pkg_dir=$runtimedir/virtcontainers/utils
	katautils=$runtimedir/pkg/katautils
	oci=$runtimedir/pkg/oci
	vc=$runtimedir/virtcontainers

	# broken packages :( :
	#vc=$runtimedir/virtcontainers

	pkgs=(
		"$device_api"
		"$device_config"
		"$device_drivers"
		"$device_manager"
		"$utils_pkg_dir"
		"$rc_pkg_dir"
                "$katautils"
                "$oci"
                "$vc"
            )
	for pkg in "${pkgs[@]}"; do
		echo building "$pkg"
		pushd "$pkg" &>/dev/null
		go build
		go test
		popd &>/dev/null
	done
}

build_working_packages
