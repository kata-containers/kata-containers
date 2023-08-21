#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o pipefail

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

IMAGE="docker.io/library/mobilenet_v1_bfloat16_fp32:latest"
DOCKERFILE="${SCRIPT_PATH}/mobilenet_v1_bfloat16_fp32_dockerfile/Dockerfile"
tensorflow_file=$(mktemp tensorflowresults.XXXXXXXXXX)
NUM_CONTAINERS="$1"
TIMEOUT="$2"
TEST_NAME="mobilenet_v1_bfloat16_fp32"
PAYLOAD_ARGS="tail -f /dev/null"
TESTDIR="${TESTDIR:-/testdir}"
# Options to control the start of the workload using a trigger-file
dst_dir="/host"
src_dir=$(mktemp --tmpdir -d tensorflowai.XXXXXXXXXX)
MOUNT_OPTIONS="type=bind,src=$src_dir,dst=$dst_dir,options=rbind:ro"
start_script="mobilenet_start.sh"
# CMD points to the script that starts the workload
CMD="$dst_dir/$start_script"
guest_trigger_file="$dst_dir/$trigger_file"
host_trigger_file="$src_dir/$trigger_file"
INITIAL_NUM_PIDS=1
CMD_FILE="cat results | grep 'Average Throughput' | wc -l"
CMD_RESULTS="cat results | grep 'Average Throughput' | cut -d':' -f2 | cut -d' ' -f2 | tr '\n' ','"

function remove_tmp_file() {
	rm -rf "${tensorflow_file}"
}

trap remove_tmp_file EXIT

function help() {
cat << EOF
Usage: $0 <count> <timeout>
	Description:
		This script launches n number of containers
		to run the MobileNet model using a Tensorflow
		container.
	Options:
		<count> : Number of containers to run.
		<timeout> : Timeout to launch the containers.
EOF
}

function create_start_script() {
	local script="${src_dir}/${start_script}"
	rm -rf "${script}"

cat <<EOF >>"${script}"
#!/bin/bash
python3.8 models/benchmarks/launch_benchmark.py --benchmark-only --framework tensorflow --model-name mobilenet_v1 --mode inference --precision bfloat16 --batch-size 100 --in-graph /mobilenet_v1_1.0_224_frozen.pb --num-intra-threads 16  --num-inter-threads 1 --verbose --\ input_height=224 input_width=224 warmup_steps=20 steps=20 \ input_layer=input output_layer=MobilenetV1/Predictions/Reshape_1 > results
EOF
	chmod +x "${script}"
}

function mobilenet_v1_bfloat16_fp32_test() {
	local CMD_EXPORT_VAR="export KMP_AFFINITY=granularity=fine,verbose,compact && export OMP_NUM_THREADS=16"

	info "Export environment variables"
	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec -d --exec-id "$(random_name)" "${i}" sh -c "${CMD_EXPORT_VAR}"
	done

	info "Running Mobilenet V1 Bfloat16 FP32 Tensorflow test"
	local pids=()
	local j=0
	for i in "${containers[@]}"; do
		$(sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "${CMD}")&
		pids["${j}"]=$!
		((j++))
	done

	# wait for all pids
	for pid in ${pids[*]}; do
		wait "${pid}"
	done

	touch "${host_trigger_file}"
	info "All containers are running the workload..."

	collect_results "${CMD_FILE}"

	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "${CMD_RESULTS}"  >> "${tensorflow_file}"
	done

	local mobilenet_results=$(cat "${tensorflow_file}" | sed 's/.$//')
	local average_mobilenet=$(echo "${mobilenet_results}" | sed 's/.$//' | sed "s/,/+/g;s/.*/(&)\/$NUM_CONTAINERS/g" | bc -l)
	local json="$(cat << EOF
	{
		"Mobilenet_v1_bfloat16_fp32": {
			"Result": "${mobilenet_results}",
			"Average": "${average_mobilenet}",
			"Units": "images/s"
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
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
	create_start_script

	info "Creating ${NUM_CONTAINERS} containers"

	for ((i=1; i<= "${NUM_CONTAINERS}"; i++)); do
		containers+=($(random_name))
		sudo -E "${CTR_EXE}" run -d --runtime "${CTR_RUNTIME}" --mount="${MOUNT_OPTIONS}" "${IMAGE}" "${containers[-1]}" sh -c "${PAYLOAD_ARGS}"
		((not_started_count--))
		info "${not_started_count} remaining containers"
	done

	metrics_json_init
	metrics_json_start_array

	# Check that the requested number of containers are running
	check_containers_are_up "${NUM_CONTAINERS}"

	# Check that the requested number of containers are running
	check_containers_are_running "${NUM_CONTAINERS}"

	# Get the initial number of pids in a single container before the workload starts
	INITIAL_NUM_PIDS=$(sudo -E "${CTR_EXE}" t metrics "${containers[-1]}" | grep pids.current | grep pids.current | xargs | cut -d ' ' -f 2)
	((INITIAL_NUM_PIDS++))

	mobilenet_v1_bfloat16_fp32_test

	metrics_json_save

	sudo rm -rf "${src_dir}"

	clean_env_ctr
}
main "$@"
