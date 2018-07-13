#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# If we fail for any reason a message will be displayed
die(){
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

# Check that kata_confing_version file is updated
# when there is any change in the kernel directory.
# If there is a change in the directory, but the config
# version is not updated, return error.
check_kata_kernel_version(){
	kernel_version_file="kernel/kata_config_version"
	modified_files=$(git diff --name-only master..)
	if echo "$modified_files" | grep "kernel/"; then
		echo "$modified_files" | grep "$kernel_version_file" || \
		die "Please bump version in $kernel_version_file"
	fi

}

install_yq() {
	path=$1
	local yq_path=${path}/yq
	local yq_pkg="github.com/mikefarah/yq"
	[ -x  "${yq_path}" ] && return

	case "$(arch)" in
	"aarch64")
		goarch=arm64
		;;

	"x86_64")
		goarch=amd64
		;;
	"*")
		echo "Arch $(arch) not supported"
		exit
		;;
	esac

	mkdir -p "${path}"

	# Workaround to get latest release from github (to not use github token).
	# Get the redirection to latest release on github.
	yq_latest_url=$(curl -Ls -o /dev/null -w %{url_effective} "https://${yq_pkg}/releases/latest")
	# The redirected url should include the latest release version
	# https://github.com/mikefarah/yq/releases/tag/<VERSION-HERE>
	yq_version=$(basename "${yq_latest_url}")


	local yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_linux_${goarch}"
	curl -o "${yq_path}" -L ${yq_url}
	chmod +x ${yq_path}
}
