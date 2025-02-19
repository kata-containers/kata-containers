#!/bin/bash
# Copyright (c) 2017-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#  Description of the test:
#  This test launches a busybox container and inside
#  memory free, memory available and total memory
#  is measured by using /proc/meminfo.

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

TEST_NAME="memory footprint inside container"
VERSIONS_FILE="${SCRIPT_PATH}/../../versions.yaml"
IMAGE='quay.io/prometheus/busybox:latest'
CMD="sleep 10; cat /proc/meminfo"
# We specify here in 'k', as that then matches the results we get from the meminfo,
# which makes later direct comparison easier.
MEMSIZE="${MEMSIZE:-$((2048*1024))}"

# this variable determines the number of attempts when a test
# result is considered not valid (a zero value or a negative value)
MAX_FAILED_ATTEMPTS=3
memtotalAvg=0
units_memtotal=""
memfreeAvg=0
units_memfree=""
memavailableAvg=0
units_memavailable=""

# count_iters: is the index of the current iteration
count_iters=0

# valid_result: if value stored is '1' the result is valid, '0' otherwise
valid_result=0

function parse_results() {
	local raw_results="${1}"

	# Variables used for sum cummulative values in the case of two or more reps.
	# and used to compute average results for 'json' output format.
	local memtotal_acu="${2:-0}"
	local memfree_acu="${3:-0}"
	local memavailable_acu="${4:-0}"

	local memtotal
	memtotal=$(echo "${raw_results}" | awk '/MemTotal/ {print $2}')
	units_memtotal=$(echo "${raw_results}" | awk '/MemTotal/ {print $3}')

	local memfree
	memfree=$(echo "${raw_results}" | awk '/MemFree/ {print $2}')
	units_memfree=$(echo "${raw_results}" | awk '/MemFree/ {print $3}')

	local memavailable
	memavailable=$(echo "${raw_results}" | awk '/MemAvailable/ {print $2}')
	units_memavailable=$(echo "${raw_results}" | awk '/MemAvailable/ {print $3}')

	# check results: if any result is zero or negative, it is considered as invalid, and the test will be repeated.
	if ((  $(echo "${memtotal} <= 0" | bc -l) )) || ((  $(echo "${memfree} <= 0" | bc -l) )) || ((  $(echo "${memavailable} <= 0" | bc -l) )); then
		MAX_FAILED_ATTEMPTS=$((MAX_FAILED_ATTEMPTS-1))
		valid_result=0
		info "Skipping invalid result:  memtotal: ${memtotal}  memfree: ${memfree}  memavailable: ${memavailable}"
		return 0
	fi

	memtotalAvg=$((memtotal+memtotal_acu))
	memfreeAvg=$((memfree+memfree_acu))
	memavailableAvg=$((memavailable+memavailable_acu))
	valid_result=1
	info "Iteration# ${count_iters}  memtotal: ${memtotal}  memfree: ${memfree}  memavailable: ${memavailable}"
}

function store_results_json() {
	metrics_json_start_array
	memtotalAvg=$(echo "scale=2; ${memtotalAvg} / ${count_iters}" | bc)
	memfreeAvg=$(echo "scale=2; ${memfreeAvg} / ${count_iters}" | bc)
	memavailableAvg=$(echo "scale=2; ${memavailableAvg} / ${count_iters}" | bc)

	local json="$(cat << EOF
	{
		"memrequest": {
			"Result" : ${MEMSIZE},
			"Units"  : "Kb"
		},
		"memtotal": {
			"Result" : ${memtotalAvg},
			"Units"  : "${units_memtotal}"
		},
		"memfree": {
			"Result" : ${memfreeAvg},
			"Units"  : "${units_memfree}"
		},
		"memavailable": {
			"Result" : ${memavailableAvg},
			"Units"  : "${units_memavailable}"
		},
		"repetitions": {
			"Result" : ${count_iters}
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save
}

function main() {
	# switch to select output format
	local num_iterations=${1:-1}
	info "Iterations: ${num_iterations}"

	# Check tools/commands dependencies
	cmds=("awk" "ctr")
	init_env
	check_cmds "${cmds[@]}"
	check_images "${IMAGE}"
	metrics_json_init
	while [  "${count_iters}" -lt "${num_iterations}" ]; do
		local output
		output=$(sudo -E "${CTR_EXE}" run --memory-limit $((MEMSIZE*1024)) --rm --runtime="${CTR_RUNTIME}" "${IMAGE}" busybox sh -c "${CMD}" 2>&1)
		parse_results "${output}" "${memtotalAvg}" "${memfreeAvg}" "${memavailableAvg}"

		# quit if number of attempts exceeds the allowed value.
		[ "${MAX_FAILED_ATTEMPTS}" -eq 0 ] && die "Max number of attempts exceeded."
		[ "${valid_result}" -eq 1 ] && count_iters=$((count_iters+1))
	done
	store_results_json
	clean_env_ctr
}

# Parameters
# @1: num_iterations {integer}
main "$@"
