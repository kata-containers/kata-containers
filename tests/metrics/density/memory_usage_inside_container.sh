#!/bin/bash
# Copyright (c) 2017-2021 Intel Corporation
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
MEMSIZE=${MEMSIZE:-$((2048*1024))}

memtotalAvg=0
units_memtotal=""
memfreeAvg=0
units_memfree=""
memavailableAvg=0
units_memavailable=""
total_iters=0
header=0


parse_results() {
	local raw_results="${1}"

	# variables that receive cummulative memory values for json case, or init to zero for csv case
	local memtotal_acu="${2:-0}"
	local memfree_acu="${3:-0}"
	local memavailable_acu="${4:-0}"

	local memtotal=$(echo "$raw_results" | awk '/MemTotal/ {print $2}')
	units_memtotal=$(echo "$raw_results" | awk '/MemTotal/ {print $3}')

	local memfree=$(echo "$raw_results" | awk '/MemFree/ {print $2}')
	units_memfree=$(echo "$raw_results" | awk '/MemFree/ {print $3}')

	local memavailable=$(echo "$raw_results" | awk '/MemAvailable/ {print $2}')
	units_memavailable=$(echo "$raw_results" | awk '/MemAvailable/ {print $3}')

	memtotalAvg=$((memtotal+memtotal_acu))
	memfreeAvg=$((memfree+memfree_acu))
	memavailableAvg=$((memavailable+memavailable_acu))

	let "total_iters=total_iters+1"

	echo "Iteration# $total_iters  memtotal: $memtotal  memfree: $memfree  memavailable: $memavailable"
}


store_results_json() {
	metrics_json_start_array
	memtotalAvg=$(( memtotalAvg / total_iters ))
	memfreeAvg=$(( memfreeAvg / total_iters ))
	memavailableAvg=$(( memavailableAvg / total_iters ))

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
			"Result" : ${total_iters}
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save
}

store_results_csv() {
	local filename=${1}

	if [ $header -eq 0 ]; then
		echo "memtotal,units_memtotal,memfree,units_memfree,memavailable,units_memavailable" > $filename
		header=1
	fi
	echo "$memtotalAvg,$units_memtotal,$memfreeAvg,$units_memfree,$memavailableAvg,$units_memavailable" >> $filename
}

function main() {
	# switch to select output format
	local output_format=${1:-json}
	local iterations=${2:-1}
	local -r csv_filename="mem-usage-inside-container-$iterations-iters-$(date +"%Y_%m_%d_%H-%M").csv"
	echo "Output format: $output_format"
	echo "Iterations: $iterations"

	# Check tools/commands dependencies
	cmds=("awk" "ctr")

	init_env
	check_cmds "${cmds[@]}"
	check_images "${IMAGE}"

	metrics_json_init

	for (( i=0; i<$iterations; i++ )) do
		local output=$(sudo -E "${CTR_EXE}" run --memory-limit $((MEMSIZE*1024)) --rm --runtime=$CTR_RUNTIME $IMAGE busybox sh -c "$CMD" 2>&1)

		if [ ${output_format} = "json" ]; then
			parse_results "${output}" "${memtotalAvg}" "${memfreeAvg}" "${memavailableAvg}"

	        # Record results per iteration
		elif [ ${output_format} = "csv" ]; then
			parse_results "${output}"
			store_results_csv ${csv_filename}
		fi
		sleep 0.5
	done

	if [ $output_format = "json" ]; then
		store_results_json
	fi

	clean_env_ctr
}

# Parameters
# @1: output format [json/csv]
# @2: iterations {integer}
main "$@"
