#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o pipefail

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../metrics/lib/common.bash"

PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"
CONTAINER_NAME="${CONTAINER_NAME:-stressng_test}"

function main() {
	local cmds=("docker")

	init_env
	check_cmds "${cmds[@]}"
	sudo -E "${CTR_EXE}" run -d --runtime "${CTR_RUNTIME}" "${IMAGE}" "${CONTAINER_NAME}" sh -c "${PAYLOAD_ARGS}"

	# Run 1 iomix stressor (mix of I/O operations) for 20 seconds with verbose output
	info "Running iomix stressor test"
	IOMIX_CMD="stress-ng --iomix 1 -t 20 -v"
	sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${CONTAINER_NAME}" sh -c "${IOMIX_CMD}"

	# Run cpu stressors and virtual memory stressors for 5 minutes
	info "Running memory stressors for 5 minutes"
	MEMORY_CMD="stress-ng --cpu 2 --vm 4 -t 5m"
	sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${CONTAINER_NAME}" sh -c "${MEMORY_CMD}"

	# Test floating point on CPU for 60 seconds
	info  "Running floating tests on CPU"
	FLOAT_CMD="stress-ng --matrix 1 -t 1m"
	sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${CONTAINER_NAME}" sh -c "${FLOAT_CMD}"

	# Runs two instances of the CPU stressors, one instance of the matrix
	info "Running instances of the CPU stressors"
	INSTANCE_CMD='stress-ng --cpu 2 --matrix 1 --mq 3 -t 3m'
	sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${CONTAINER_NAME}" sh -c "${INSTANCE_CMD}"

	clean_env_ctr
}

main "$@"
