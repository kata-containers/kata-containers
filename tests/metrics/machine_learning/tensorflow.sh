#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

IMAGE="docker.io/library/tensorflow:latest"
DOCKERFILE="${SCRIPT_PATH}/tensorflow_dockerfile/Dockerfile"
BATCH_SIZE="512"
NUM_BATCHES="300"
CMD_RESULT="cd benchmarks/scripts/tf_cnn_benchmarks/ && cat result"
CMD_FILE="cat benchmarks/scripts/tf_cnn_benchmarks/result | grep 'total images' | wc -l"
tensorflow_file=$(mktemp tensorflowresults.XXXXXXXXXX)
NUM_CONTAINERS="$1"
TIMEOUT="$2"
TEST_NAME="tensorflow"
PAYLOAD_ARGS="tail -f /dev/null"

function remove_tmp_file() {
	rm -rf "${tensorflow_file}"
}

trap remove_tmp_file EXIT

function help() {
cat << EOF
Usage: $0 <count> <timeout>
	Description:
		This script launches n number of containers
		to run the tf cnn benchmarks using a Tensorflow
		container.
	Options:
		<count> : Number of containers to run.
		<timeout> : Timeout to launch the containers.
EOF
}

function resnet50_test() {
	local CMD_RUN="cd benchmarks/scripts/tf_cnn_benchmarks/ && python tf_cnn_benchmarks.py -data_format=NHWC --device cpu --batch_size=${BATCH_SIZE} --num_batches=${NUM_BATCHES} > result"
	info "Running Resnet50 Tensorflow test"
	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec -d --exec-id "$(random_name)" "${i}" sh -c "${CMD_RUN}"
	done

	for i in "${containers[@]}"; do
		check_file=$(sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "${CMD_FILE}")
		retries="200"
		for j in $(seq 1 "${retries}"); do
			[ "${check_file}" -eq 1 ] && break
			sleep 1
		done
	done

	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "${CMD_RESULT}"  >> "${tensorflow_file}"
	done

	local resnet50_results=$(cat "${tensorflow_file}" | grep "total images/sec" | cut -d ":" -f2 | sed -e 's/^[ \t]*//' | tr '\n' ',' | sed 's/.$//')
	local average_resnet50=$(echo "${resnet50_results}" | sed "s/,/+/g;s/.*/(&)\/$NUM_CONTAINERS/g" | bc -l)

	local json="$(cat << EOF
	{
		"Resnet50": {
			"Result": "${resnet50_results}",
			"Average": "${average_resnet50}",
			"Units": "s"
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
}

function axelnet_test() {
	local CMD_RUN="cd benchmarks/scripts/tf_cnn_benchmarks/ && python tf_cnn_benchmarks.py --num_batches=${NUM_BATCHES} --device=cpu --batch_size=${BATCH_SIZE} --forward_only=true --model=alexnet --data_format=NHWC > result"
	info "Running AxelNet Tensorflow test"
	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec -d --exec-id "$(random_name)" "${i}" sh -c "${CMD_RUN}"
	done

	for i in "${containers[@]}"; do
		check_file=$(sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "${CMD_FILE}")
		retries="200"
		for j in $(seq 1 "${retries}"); do
			[ "${check_file}" -eq 1 ] && break
			sleep 1
		done
	done

	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "${CMD_RESULT}"  >> "${tensorflow_file}"
	done

	local axelnet_results=$(cat "${tensorflow_file}" | grep "total images/sec" | cut -d ":" -f2 | sed -e 's/^[ \t]*//' | tr '\n' ',' | sed 's/.$//')
	local average_axelnet=$(echo "${axelnet_results}" | sed "s/,/+/g;s/.*/(&)\/$NUM_CONTAINERS/g" | bc -l)

	local json="$(cat << EOF
	{
		"AxelNet": {
			"Result": "${axelnet_results}",
			"Average": "${average_axelnet}",
			"Units": "s"
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
}

function check_containers_are_up() {
	local containers_launched=0
	for i in $(seq "${TIMEOUT}") ; do
		info "Verify that the containers are running"
		containers_launched="$(sudo ${CTR_EXE} t list | grep -c "RUNNING")"
		[ "${containers_launched}" -eq "${NUM_CONTAINERS}" ] && break
		sleep 1
		[ "${i}" == "${TIMEOUT}" ] && return 1
	done
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

	# Check tools/commands dependencies
	cmds=("awk" "docker" "bc")
	check_cmds "${cmds[@]}"
	check_ctr_images "${IMAGE}" "${DOCKERFILE}"

	init_env
	info "Creating ${NUM_CONTAINERS} containers"

	for ((i=1; i<= "${NUM_CONTAINERS}"; i++)); do
		containers+=($(random_name))
		sudo -E "${CTR_EXE}" run -d --runtime "${CTR_RUNTIME}" "${IMAGE}" "${containers[-1]}" sh -c "${PAYLOAD_ARGS}"
		((not_started_count--))
		info "$not_started_count remaining containers"
	done

	metrics_json_init
	metrics_json_start_array

	# Check that the requested number of containers are running
	check_containers_are_up

	resnet50_test

	axelnet_test

	metrics_json_save

	clean_env_ctr
}
main "$@"
