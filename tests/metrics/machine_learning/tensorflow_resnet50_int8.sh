#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o pipefial

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

IMAGE="docker.io/library/resnet50int8:latest"
DOCKERFILE="${SCRIPT_PATH}/resnet50_int8_dockerfile/Dockerfile"
tensorflow_file=$(mktemp tensorflowresults.XXXXXXXXXX)
NUM_CONTAINERS="$1"
TIMEOUT="$2"
TEST_NAME="tensorflow-resnet50int8"
PAYLOAD_ARGS="tail -f /dev/null"
TESTDIR="${TESTDIR:-/testdir}"
# Options to control the start of the workload using a trigger-file
dst_dir="/host"
src_dir=$(mktemp --tmpdir -d tensorflowresnet50int8.XXXXXXXXXX)
MOUNT_OPTIONS="type=bind,src=$src_dir,dst=$dst_dir,options=rbind:ro"
start_script="resnet50int8_start.sh"
# CMD points to the script that starts the workload
# export DNNL_MAX_CPU_ISA=AVX512_CORE_AMX
CMD="export KMP_AFFINITY=granularity=fine,verbose,compact && export OMP_NUM_THREADS=16 && $dst_dir/$start_script"
guest_trigger_file="$dst_dir/$trigger_file"
host_trigger_file="$src_dir/$trigger_file"
INITIAL_NUM_PIDS=1
CMD_FILE="cat results | grep 'Throughput' | wc -l"
CMD_RESULTS="cat results | grep 'Throughput' | cut -d':' -f2 | cut -d' ' -f2 | tr '\n' ','"

function remove_tmp_file() {
	rm -rf "${tensorflow_file}"
}

trap remove_tmp_file EXIT

function help() {
cat << EOF
Usage: $0 <count> <timeout>
	Description:
		This script launches n number of containers
		to run the ResNet50 int8 using a Tensorflow
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
python3.10 models/benchmarks/launch_benchmark.py --benchmark-only --framework tensorflow --model-name resnet50  --precision int8 --mode inference --in-graph /resnet50_int8_pretrained_model.pb --batch-size 116 --num-intra-threads 16 >> results
EOF
	chmod +x "${script}"
}

function resnet50_int8_test() {
	info "Running ResNet50 Int8 Tensorflow test"
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

	local resnet50_int8_results=$(cat "${tensorflow_file}" | sed 's/.$//')
	local average_resnet50_int8=$(echo "${resnet50_int8_results}" | sed 's/.$//'| sed "s/,/+/g;s/.*/(&)\/2/g" | bc -l)

	local json="$(cat << EOF
	{
		"ResNet50Int8": {
			"Result": "${resnet50_int8_results}",
			"Average": "${average_resnet50_int8}",
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
		info "$not_started_count remaining containers"
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

	resnet50_int8_test

	metrics_json_save

	sudo rm -rf "${src_dir}"

	clean_env_ctr
}
main "$@"
