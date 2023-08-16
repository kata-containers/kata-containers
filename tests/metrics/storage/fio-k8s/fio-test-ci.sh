#!/bin/bash
#
# Copyright (c) 2022-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../lib/common.bash"
FIO_PATH="${GOPATH}/src/github.com/kata-containers/kata-containers/tests/metrics/storage/fio-k8s"
TEST_NAME="${TEST_NAME:-fio}"

function main() {
	cmds=("bc" "jq")
	check_cmds "${cmds[@]}"
	check_processes
	init_env

	pushd "${FIO_PATH}"
		[ -z "${KATA_HYPERVISOR}" ] && die "Hypervisor ID is missing."
		[ "${KATA_HYPERVISOR}" != "qemu" ] && [ "${KATA_HYPERVISOR}" != "clh" ] && die "Hypervisor not recognized: ${KATA_HYPERVISOR}"

		echo "INFO: Running K8S FIO test using ${KATA_HYPERVISOR} hypervisor"
		make "test-${KATA_HYPERVISOR}"
	popd

	test_result_file="${FIO_PATH}/cmd/fiotest/test-results/kata/randrw-sync.job/output.json"

	metrics_json_init
	local read_io=$(cat $test_result_file | grep io_bytes | head -1 | sed 's/[[:blank:]]//g' | cut -f2 -d ':' | cut -f1 -d ',')
	local read_bw=$(cat $test_result_file | grep bw_bytes | head -1 | sed 's/[[:blank:]]//g' | cut -f2 -d ':' | cut -f1 -d ',')
	local read_90_percentile=$(cat $test_result_file | grep 90.000000 | head -1 | sed 's/[[:blank:]]//g' | cut -f2 -d ':' | cut -f1 -d ',')
	local read_95_percentile=$(cat $test_result_file | grep 95.000000 | head -1 | sed 's/[[:blank:]]//g' | cut -f2 -d ':' | cut -f1 -d ',')
	local write_io=$(cat $test_result_file | grep io_bytes | head -2 | tail -1 | sed 's/[[:blank:]]//g' | cut -f2 -d ':' | cut -f1 -d ',')
	local write_bw=$(cat $test_result_file | grep bw_bytes | head -2 | tail -1 | sed 's/[[:blank:]]//g' | cut -f2 -d ':' | cut -f1 -d ',')
	local write_90_percentile=$(cat $test_result_file | grep 90.000000 | head -2 | tail -1 | sed 's/[[:blank:]]//g' | cut -f2 -d ':' | cut -f1 -d ',')
	local write_95_percentile=$(cat $test_result_file | grep 95.000000 | head -2 | tail -1 | sed 's/[[:blank:]]//g' | cut -f2 -d ':' | cut -f1 -d ',')

	metrics_json_start_array
	local json="$(cat << EOF
	{
		"readio": {
			"Result" : $read_io,
			"Units" : "bytes"
		},
		"readbw": {
			"Result" : $read_bw,
			"Units" : "bytes/sec"
		},
		"read90percentile": {
			"Result" : $read_90_percentile,
			"Units" : "ns"
		},
		"read95percentile": {
			"Result" : $read_95_percentile,
			"Units" : "ns"
		},
		"writeio": {
			"Result" : $write_io,
			"Units" : "bytes"
		},
		"writebw": {
			"Result" : $write_bw,
			"Units" : "bytes/sec"
		},
		"write90percentile": {
			"Result" : $write_90_percentile,
			"Units" : "ns"
		},
		"write95percentile": {
			"Result" : $write_95_percentile,
			"Units" : "ns"
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save

	check_processes
}

main "$@"
