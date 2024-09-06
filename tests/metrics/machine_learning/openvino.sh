#!/bin/bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description of the test:
# This test runs the 'openvino benchmark'
# https://openbenchmarking.org/test/pts/openvino

set -o pipefail

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

# TEST_NAME is required to collect results and name the workload container.
TEST_NAME="openvino-bench"

WORKLOAD="phoronix-test-suite batch-run openvino"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"

IMAGE="docker.io/library/pts-openvino:latest"
DOCKERFILE="${SCRIPT_PATH}/openvino-dockerfile/Dockerfile"

TMP_DIR=$(mktemp --tmpdir -d openvino.XXXXXXXXXX)
KATA_PERF_CONFIG="${TMP_DIR}/openvino_config.toml"
TEST_RESULTS_FNAME="${TMP_DIR}/openvino-results.json"

# Variable used to store the initial configuration file name.
# This file is again pointed to by kata once the script finishes.
KATA_INITIAL_CONFIG_FNAME=""

function restore_kata_config() {
	rm -rf "${TMP_DIR}"
	set_kata_config_file "${KATA_INITIAL_CONFIG_FNAME}"
}
trap restore_kata_config EXIT

# Show help about this script
function help(){
cat << EOF
Usage: $0
   Description:
       Runs openvino benchmark.
EOF
}

function save_config() {
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"image": "${IMAGE}",
		"units": "ms",
		"mode": "Lower Is Better",
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Config"
}

function main() {
	local cmds=("docker")
	local RES_DIR="/var/lib/phoronix-test-suite/test-results"

	# Check tools/commands dependencies
 	init_env
	check_cmds "${cmds[@]}"
 	check_ctr_images "$IMAGE" "$DOCKERFILE"

	clean_cache

	# Configure Kata to use the maximum number of available CPUs
	# and to use the available free memory.
	get_current_kata_config_file KATA_INITIAL_CONFIG_FNAME
	set_kata_configuration_performance "${KATA_PERF_CONFIG}"

	# Launch container.
	sudo -E "${CTR_EXE}" run -d --runtime "${CTR_RUNTIME}" "${IMAGE}" "${TEST_NAME}" sh -c "${PAYLOAD_ARGS}"

	# Run the test.
	sudo -E "${CTR_EXE}" t exec -t --exec-id "$(random_name)" "${TEST_NAME}" sh -c "${WORKLOAD}"

	results_fname=$(sudo -E "${CTR_EXE}" t exec --exec-id $(random_name) ${TEST_NAME} sh -c "ls ${RES_DIR}")
	SAVE_RESULTS_CMD="phoronix-test-suite result-file-to-json ${results_fname}"

	# Save results.
	sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${TEST_NAME}" sh -c "${SAVE_RESULTS_CMD}"

	# Extract results.
	sudo -E "${CTR_EXE}" t exec --exec-id "${RANDOM}" "${TEST_NAME}" sh -c "cat /root/${results_fname}.json" > "${TEST_RESULTS_FNAME}"

	cat <<< $(jq 'del(.systems[].data)' "${TEST_RESULTS_FNAME}") > "${TEST_RESULTS_FNAME}"
	local results="$(cat "${TEST_RESULTS_FNAME}")"

	metrics_json_init
	save_config
	metrics_json_start_array
	metrics_json_add_array_element "${results}"
	metrics_json_end_array "Results"
	metrics_json_save
	clean_env_ctr
}

main "$@"
