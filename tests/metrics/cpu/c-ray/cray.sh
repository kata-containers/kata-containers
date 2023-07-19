#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../lib/common.bash"

TEST_NAME="cray"
IMAGE="docker.io/library/cray:latest"
DOCKERFILE="${SCRIPT_PATH}/Dockerfile"
CMD="cd c-ray-1.1 && ./c-ray-mt -t 32 -s 1024x768 -r 8 -i sphfract -o output.ppm 2>&1 | tee -a output.txt && cat output.txt"
cray_file=$(mktemp crayresults.XXXXXXXXXX)

function remove_tmp_file() {
	rm -rf "${cray_file}"
}

trap remove_tmp_file EXIT

function main() {
	# Check tools/commands dependencies
	cmds=("awk" "docker")
	init_env
	check_cmds "${cmds[@]}"
	check_ctr_images "$IMAGE" "$DOCKERFILE"

	sudo -E "${CTR_EXE}" run --rm --runtime="${CTR_RUNTIME}" "${IMAGE}" test sh -c "${CMD}" > "${cray_file}"
	metrics_json_init
	results=$(cat "${cray_file}" | grep seconds | awk '{print $3}' | head -n 1)
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"rendering": {
			"Result": ${results},
			"Units": "s"
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save

	clean_env_ctr
}

main "$@"
