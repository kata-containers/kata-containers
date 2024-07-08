#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o pipefail

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../metrics/lib/common.bash"

NUM_CONTAINERS="$1"
TIMEOUT_LAUNCH="$2"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"
IMAGE="${IMAGE:-quay.io/prometheus/busybox:latest}"

# Show help about this script
help(){
cat << EOF
Usage: $0 <count> <timeout>
	Description:
		This script launches n number of containers.
	Options:
		<count> : Number of containers to run.
		<timeout>: Timeout to launch the containers.
EOF
}

function main() {
	# Verify enough arguments
	if [ $# != 2 ]; then
		echo >&2 "error: Not enough arguments [$@]"
		help
		exit 1
	fi

	local i=0
	local containers=()
	local not_started_count="${NUM_CONTAINERS}"

	clean_env_ctr
	sudo -E "${CTR_EXE}" i pull "${IMAGE}"

	info "Creating ${NUM_CONTAINERS} containers"

	for ((i=1; i<= "${NUM_CONTAINERS}"; i++)); do
		containers+=($(random_name))
		sudo -E "${CTR_EXE}" run -d --runtime "${CTR_RUNTIME}" "${IMAGE}" "${containers[-1]}" sh -c "${PAYLOAD_ARGS}"
		((not_started_count--))
		info "$not_started_count remaining containers"
	done

	# Check that the requested number of containers are running
	check_containers_are_up "${NUM_CONTAINERS}"

	# Check that the requested number of containers are running
	check_containers_are_running "${NUM_CONTAINERS}"

	clean_env_ctr
}

main "$@"
