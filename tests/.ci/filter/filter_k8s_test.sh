#!/bin/bash
#
# Copyright (c) 2019 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

GOPATH_LOCAL="${GOPATH%%:*}"
KATA_DIR="${GOPATH_LOCAL}/src/github.com/kata-containers"
TEST_DIR="${KATA_DIR}/tests"
CI_DIR="${TEST_DIR}/.ci"

K8S_FILTER_FLAG="kubernetes"

source "${CI_DIR}/lib.sh"

main()
{
	local K8S_CONFIG_FILE="$1"
	local K8S_TEST_UNION="$2"
	local result=()

	mapfile -d " " -t _K8S_TEST_UNION <<< "${K8S_TEST_UNION}"

	# install yq if not exist
        ${CI_DIR}/install_yq.sh > /dev/null

        local K8S_SKIP_UNION=$("${GOPATH_LOCAL}/bin/yq" read "${K8S_CONFIG_FILE}" "${K8S_FILTER_FLAG}")
        [ "${K8S_SKIP_UNION}" == "null" ] && return
        mapfile -t _K8S_SKIP_UNION <<< "${K8S_SKIP_UNION}"

	for TEST_ENTRY in "${_K8S_TEST_UNION[@]}"
	do
		local flag="false"
		for SKIP_ENTRY in "${_K8S_SKIP_UNION[@]}"
		do
			SKIP_ENTRY="${SKIP_ENTRY#- }.bats"
			[ "$SKIP_ENTRY" == "$TEST_ENTRY" ] && flag="true"
		done
		[ "$flag" == "false" ] && result+=("$TEST_ENTRY")
	done
	echo ${result[@]}
}

main "$@"
