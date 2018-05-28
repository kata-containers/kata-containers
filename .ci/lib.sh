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
