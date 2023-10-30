#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description of the test:
# This test runs the 'fio benchmark' on kata containers
# https://fio.readthedocs.io/en/latest/

set -o pipefail

# FIO variable settings

# io-types supported:
# read, write, randread, randwrite, randrw, readwrite
io_type="read"
block_size="4k"
num_jobs="4"

# FIO default settings
readonly ioengine="libaio"
readonly rate_process="linear"
readonly disable_buffered="1"
readonly iodepth="8"
readonly runtime="10s"
# ramp time
readonly rt="10s"
readonly fname="test.fio"
readonly workload_dir="/"
readonly workload_file="${workload_dir}${fname}"
readonly workload_size="10G"
readonly summary_file_local="/results.json"

# Show help about this script
function help() {
cat << EOF
Usage: $0 <count>
   Description:
	Runs FIO test using ctr to excercise IO in kata containers.

	Params: <Operation> <io-engine>

	Operations are:
		run-read-4k
		run-write-4k
		run-randread-4k
		run-randwrite-4k
		run-read-64k
		run-write-64k
		run-randread-64k
		run-randwrite-64k

	<Operation>: [Mandatory]
	<io-engine> : [Optional] Any of the FIO supported ioengines, default: libaio.
EOF
}

# Run from the host
function setup_workload() {
        # create workload file:
	if [ ! -f ${workload_file} ]; then
	        pushd "${workload_dir}" > /dev/null 2>&1
		dd if=/dev/urandom of="${workload_file}" bs=64M count=160 > /dev/null 2>&1
	fi
}

# Run inside container
function launch_workload() {
	# the parameters used in the test_name are accesible globally
	local test_name="${io_type}_${block_size}_nj-${num_jobs}_${rate_process}_iodepth-${iodepth}_io-direct-${disable_buffered}"

	setup_workload
	rm -f "${summary_file_local}" >/dev/null 2>&1
        fio \
	--name="${test_name}" \
	--output-format="json" \
	--filename="${workload_file}" \
	--size="${workload_size}" \
	--rate_process="${rate_process}" \
	--runtime="${runtime}" \
	--ioengine="${ioengine}" \
	--rw="${io_type}" \
	--direct="${disable_buffered}" \
	--numjobs="${num_jobs}" \
	--blocksize="${block_size}" \
	--ramp_time="${rt}" \
	--iodepth="${iodepth}" \
	--gtod_reduce="1" \
	--randrepeat="1" \
	--output "${summary_file_local}" >/dev/null 2>&1
}

function print_latest_results() {
	[ ! -f "${summary_file_local}" ] && echo "Error: no results to display; you must run a test before requesting results display" && exit 1
	cat "${summary_file_local}"
}

function delete_workload() {
	rm -f "${workload_file}" > /dev/null 2>&1
}

function main() {
        local action="${1:-}"
	num_jobs="${2:-1}"

	[[ ! ${num_jobs} =~ ^[0-9]+$ ]] && die "The number of jobs must be a positive integer"

        case "${action}" in
                run-read-4k) launch_workload ;;
                run-read-64k) block_size="64k" && launch_workload ;;

                run-write-4k) io_type="write" && launch_workload ;;
                run-write-64k) block_size="64k" && io_type="write" && launch_workload ;;

		run-randread-4k) io_type="randread" && launch_workload ;;
		run-randread-64k) block_size="64k" && io_type="randread" && launch_workload ;;

		run-randwrite-4k) io_type="randwrite" && launch_workload ;;
		run-randwrite-64k) block_size="64k" && io_type="randwrite" && launch_workload ;;

		print-latest-results) print_latest_results ;;
		delete-workload) delete_workload ;;

                *) >&2 echo "Invalid argument" ; help ; exit 1;;
        esac
}

main "$@"
