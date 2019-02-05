#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/../../lib.sh"

test_config_file="${cidir}/configuration_firecracker.yaml"

describe_skip_flag="docker.Describe"
context_skip_flag="docker.Context"
it_skip_flag="docker.It"

# value for '-skip' in ginkgo
_skip_options=()

filter_and_build() {
	local dependency="$1"
	local array_docker=$("${GOPATH}/bin/yq" read "${test_config_file}" "${dependency}")
	[ "${array_docker}" = "null" ] && return
	mapfile -t _array_docker <<< "${array_docker}"
	for entry in "${_array_docker[@]}"
	do
		_skip_options+=("${entry#- }|")
	done
}

main() {
	# Check if yq is installed
	[ -z "$(command -v yq)" ] && install_yq

	# Build skip option based on Describe block
	filter_and_build "${describe_skip_flag}"

	# Build skip option based on context block
	filter_and_build "${context_skip_flag}"

	# Build skip option based on it block
	filter_and_build "${it_skip_flag}"

	skip_options=$(IFS= ; echo "${_skip_options[*]}")

	echo "${skip_options%|}"
}

main
