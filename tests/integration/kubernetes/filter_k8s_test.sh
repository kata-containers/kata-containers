#!/bin/bash
#
# Copyright (c) 2019 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "${script_dir}/../../.." && pwd)"

GOPATH_LOCAL="${GOPATH%%:*}"
K8S_FILTER_FLAG="kubernetes"

main()
{
	local K8S_CONFIG_FILE="$1"
	local K8S_TEST_UNION="$2"
	local result=()

	mapfile -d " " -t _K8S_TEST_UNION <<< "${K8S_TEST_UNION}"

	if [ ! -f ${GOPATH_LOCAL}/bin/yq ]; then
		${repo_root_dir}/ci/install_yq.sh > /dev/null
	fi

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
