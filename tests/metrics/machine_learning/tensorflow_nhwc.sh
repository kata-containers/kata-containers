#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o pipefail

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

IMAGE="docker.io/library/tensorflow_nhwc:latest"
DOCKERFILE="${SCRIPT_PATH}/tensorflow_nhwc_dockerfile/Dockerfile"
BATCH_SIZE="100"
NUM_BATCHES="100"
resnet_tensorflow_file=$(mktemp resnettensorflowresults.XXXXXXXXXX)
alexnet_tensorflow_file=$(mktemp alexnettensorflowresults.XXXXXXXXXX)
NUM_CONTAINERS="$1"
TIMEOUT="$2"
TEST_NAME="tensorflow_nhwc"
PAYLOAD_ARGS="tail -f /dev/null"
# Options to control the start of the workload using a trigger-file
dst_dir="/host"
src_dir=$(mktemp --tmpdir -d tensorflow.XXXXXXXXXX)
MOUNT_OPTIONS="type=bind,src=$src_dir,dst=$dst_dir,options=rbind:ro"
# CMD points to the script that starts the workload
alexnet_start_script="alexnet_start.sh"
resnet_start_script="resnet_start.sh"
CMD_RESNET="$dst_dir/$resnet_start_script"
CMD_ALEXNET="$dst_dir/$alexnet_start_script"
INITIAL_NUM_PIDS=1
ALEXNET_FILE="alexnet_results"
ALEXNET_CHECK_FILE_CMD="cat /${ALEXNET_FILE} | grep 'total images' | wc -l"
RESNET_FILE="resnet_results"
RESNET_CHECK_FILE_CMD="cat /${RESNET_FILE} | grep 'total images' | wc -l"

function remove_tmp_file() {
	rm -rf "${resnet_tensorflow_file}" "${alexnet_tensorflow_file}"
	rm -rf "${src_dir}"
	clean_env_ctr
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

function create_resnet_start_script() {
	local script="${src_dir}/${resnet_start_script}"
	rm -rf "${script}"

cat <<EOF >>"${script}"
#!/bin/bash
pushd "benchmarks/scripts/tf_cnn_benchmarks"
python tf_cnn_benchmarks.py -data_format=NHWC --device cpu --batch_size=${BATCH_SIZE} --num_batches=${NUM_BATCHES} > "/${RESNET_FILE}"
EOF
	chmod +x "${script}"
}

function create_alexnet_start_script() {
	local script="${src_dir}/${alexnet_start_script}"
	rm -rf "${script}"

cat <<EOF >>"${script}"
#!/bin/bash
pushd "benchmarks/scripts/tf_cnn_benchmarks"
python tf_cnn_benchmarks.py --num_batches=${NUM_BATCHES} --device=cpu --batch_size=${BATCH_SIZE} --forward_only=true --model=alexnet --data_format=NHWC > "/${ALEXNET_FILE}"
EOF
	chmod +x "${script}"
}

function launch_workload() {
	WORKLOAD=${1}
	[[ -z ${WORKLOAD} ]] && die "Container workload is missing"

	local pids=()
	local j=0
	for i in "${containers[@]}"; do
		$(sudo -E "${CTR_EXE}" t exec -d --exec-id "$(random_name)" "${i}" sh -c "${WORKLOAD}")&
		pids["${j}"]=$!
		((j++))
	done

	# wait for all pids
	for pid in ${pids[*]}; do
		wait "${pid}"
	done
}

function tensorflow_nhwc_test() {
	# Resnet section
	info "Running TF-Resnet test"
	launch_workload "${CMD_RESNET}"
	collect_results "${RESNET_CHECK_FILE_CMD}"

	# Alexnet section
	info "Running TF-Alexnet test"
	launch_workload "${CMD_ALEXNET}"
	collect_results "${ALEXNET_CHECK_FILE_CMD}"

	info "Tensorflow workload completed"
	# Retrieving results
	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "cat /${RESNET_FILE}"  >> "${resnet_tensorflow_file}"
		sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "cat /${ALEXNET_FILE}"  >> "${alexnet_tensorflow_file}"
	done

	# Parsing resnet results
	local resnet_results=$(cat "${resnet_tensorflow_file}" | grep "total images/sec" | cut -d ":" -f2 | sed -e 's/^[ \t]*//' | tr '\n' ',' | sed 's/.$//')
	local res_sum="$(sed -e 's/,/\n/g' <<< ${resnet_results} | awk 'BEGIN {total=0} {total += $1} END {print total}')"
	local num_elements="$(awk '{print NF}' FS=',' <<<${resnet_results})"
	local average_resnet="$(echo "scale=2 ; ${res_sum} / ${num_elements}" | bc)"

	# Parsing alexnet results
	local alexnet_results=$(cat "${alexnet_tensorflow_file}" | grep "total images/sec" | cut -d ":" -f2 | sed -e 's/^[ \t]*//' | tr '\n' ',' | sed 's/.$//')
	local alex_sum="$(sed -e 's/,/\n/g' <<< ${alexnet_results} | awk 'BEGIN {total=0} {total += $1} END {print total}')"
	num_elements="$(awk '{print NF}' FS=',' <<< ${alexnet_results})"
	local average_alexnet="$(echo " scale=2 ; ${alex_sum} / ${num_elements}" | bc)"

	# writing json results file
	local json="$(cat << EOF
	{
		"resnet": {
			"Result": ${resnet_results},
			"Average": ${average_resnet},
			"Units": "images/s"
		},
		"alexnet": {
			"Result": ${alexnet_results},
			"Average": ${average_alexnet},
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
	if [ "$#" -lt 2 ]; then
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
	create_resnet_start_script
	create_alexnet_start_script

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
	tensorflow_nhwc_test
	metrics_json_save
}

main "$@"
