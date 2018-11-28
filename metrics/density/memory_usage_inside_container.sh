#!/bin/bash
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#  Description of the test:
#  This test launches an alpine container and inside
#  memory free, memory available and total memory
#  is measured by using /proc/meminfo.

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

TEST_NAME="memory footprint inside container"
VERSIONS_FILE="${SCRIPT_PATH}/../../versions.yaml"
ALPINE_VERSION=$("${GOPATH}/bin/yq" read "$VERSIONS_FILE" "docker_images.alpine.version")
IMAGE="alpine:$ALPINE_VERSION"
CMD="sleep 10; cat /proc/meminfo"
# We specify here in 'k', as that then matches the results we get from the meminfo,
# which makes later direct comparison easier.
MEMSIZE=${MEMSIZE:-$((2048*1024))}

function main() {
	# Check tools/commands dependencies
	cmds=("awk" "docker")

	init_env
	check_cmds "${cmds[@]}"

	metrics_json_init

	local output=$(docker run -m ${MEMSIZE}k --rm --runtime=$RUNTIME $IMAGE sh -c "$CMD")

	# Save configuration
	metrics_json_start_array

	local memtotal=$(echo "$output" | awk '/MemTotal/ {print $2}')
	local units_memtotal=$(echo "$output" | awk '/MemTotal/ {print $3}')
	local memfree=$(echo "$output" | awk '/MemFree/ {print $2}')
	local units_memfree=$(echo "$output" | awk '/MemFree/ {print $3}')
	local memavailable=$(echo "$output" | awk '/MemAvailable/ {print $2}')
	local units_memavailable=$(echo "$output" | awk '/MemAvailable/ {print $3}')

	local json="$(cat << EOF
	{
		"memrequest": {
			"Result" : $MEMSIZE,
			"Units"  : "Kb"
		},
		"memtotal": {
			"Result" : $memtotal,
			"Units"  : "$units_memtotal"
		},
		"memfree": {
			"Result" : $memfree,
			"Units"  : "$units_memfree"
		},
		"memavailable": {
			"Result" : $memavailable,
			"Units"  : "$units_memavailable"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save
	clean_env
}

main "$@"
