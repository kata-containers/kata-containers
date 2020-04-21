#!/bin/bash
#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -e

test_config_file="$1"

GOPATH_LOCAL="${GOPATH%%:*}"
kata_dir="${GOPATH_LOCAL}/src/github.com/kata-containers"
test_dir="${kata_dir}/tests"
ci_dir="${test_dir}/.ci"

test_filter_flag="test"

_test_union=()

source "${ci_dir}/lib.sh"

main()
{
	# install yq if not exist
	${ci_dir}/install_yq.sh
	local array_test=$("${GOPATH_LOCAL}/bin/yq" read "${test_config_file}" "${test_filter_flag}")
	[ "${array_test}" = "null" ] && return
	mapfile -t _array_test <<< "${array_test}"
	for entry in "${_array_test[@]}"
	do
		_test_union+=("${entry#- }")
	done
	test_union=$(IFS=" "; echo "${_test_union[*]}")
	echo "${test_union}"
}

main
