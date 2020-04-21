#!/usr/bin/env bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

test_config_file="$1"

it_filter_flag="podman.It"

# value for '--focus' in ginkgo
_focus_options=()

GOPATH_LOCAL="${GOPATH%%:*}"
kata_dir="${GOPATH_LOCAL}/src/github.com/kata-containers"
test_dir="${kata_dir}/tests"
ci_dir="${test_dir}/.ci"
source "${ci_dir}/lib.sh"

filter_and_build()
{
	local dependency="$1"
	local array_podman=$("${GOPATH_LOCAL}/bin/yq" read "${test_config_file}" "${dependency}")
	[ "${array_podman}" = "null" ] && return
	mapfile -t _array_podman <<< "${array_podman}"
	for entry in "${_array_podman[@]}"
	do
		_focus_options+=("${entry#- }|")
	done
}

main()
{
	# install yq if not exist
	"${ci_dir}"/install_yq.sh

	# build focus option based on it block
	filter_and_build "${it_filter_flag}"

	focus_options=$(IFS= ; echo "${_focus_options[*]}")

	echo "${focus_options%|}"
}

main
