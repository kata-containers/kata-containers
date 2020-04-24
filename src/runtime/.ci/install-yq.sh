#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# If we fail for any reason a message will be displayed
die() {
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

# Install the yq yaml query package from the mikefarah github repo
# Install via binary download, as we may not have golang installed at this point
function install_yq() {
	GOPATH=${GOPATH:-${HOME}/go}
	local yq_path="${GOPATH}/bin/yq"
	local yq_pkg="github.com/mikefarah/yq"
	[ -x  "${GOPATH}/bin/yq" ] && return

	read -r -a sysInfo <<< "$(uname -sm)"

	case "${sysInfo[0]}" in
	"Linux" | "Darwin")
		goos="${sysInfo[0],}"
		;;
	"*")
		die "OS ${sysInfo[0]} not supported"
		;;
	esac

	case "${sysInfo[1]}" in
	"aarch64")
		goarch=arm64
		;;
	"ppc64le")
		goarch=ppc64le
		;;
	"x86_64")
		goarch=amd64
		;;
	"s390x")
		goarch=s390x
		;;
	"*")
		die "Arch ${sysInfo[1]} not supported"
		;;
	esac

	mkdir -p "${GOPATH}/bin"

	# Check curl
	if ! command -v "curl" >/dev/null; then
		die "Please install curl"
	fi

	local yq_version=3.1.0

	local yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_${goos}_${goarch}"
	curl -o "${yq_path}" -LSsf ${yq_url}
	[ $? -ne 0 ] && die "Download ${yq_url} failed"
	chmod +x ${yq_path}

	if ! command -v "${yq_path}" >/dev/null; then
		die "Cannot not get ${yq_path} executable"
	fi
}

install_yq
