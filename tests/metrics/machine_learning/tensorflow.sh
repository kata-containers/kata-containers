#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

#set -e
set -x

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

IMAGE="docker.io/library/tensorflow:latest"
DOCKERFILE="${SCRIPT_PATH}/tensorflow_dockerfile/Dockerfile"
BATCH_SIZE="512"
NUM_BATCHES="300"
resnet_tensorflow_file=$(mktemp resnettensorflowresults.XXXXXXXXXX)
alexnet_tensorflow_file=$(mktemp alexnettensorflowresults.XXXXXXXXXX)
NUM_CONTAINERS="$1"
TIMEOUT="$2"
TEST_NAME="tensorflow"
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
timeout=600
INITIAL_NUM_PIDS=1
CMD_FILE="cat alexnet_results | grep 'total images' | wc -l"
RESNET_CMD_FILE="cat resnet_results | grep 'total images' | wc -l"

function remove_tmp_file() {
	rm -rf "${resnet_tensorflow_file}" "${alexnet_tensorflow_file}"
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
python benchmarks/scripts/tf_cnn_benchmarks/tf_cnn_benchmarks.py -data_format=NHWC --device cpu --batch_size=${BATCH_SIZE} --num_batches=${NUM_BATCHES} > resnet_results
EOF
	chmod +x "${script}"
}

function create_alexnet_start_script() {
	local script="${src_dir}/${alexnet_start_script}"
	rm -rf "${script}"

cat <<EOF >>"${script}"
#!/bin/bash
python benchmarks/scripts/tf_cnn_benchmarks/tf_cnn_benchmarks.py --num_batches=100 --device=cpu --batch_size=100 --forward_only=true --model=alexnet --data_format=NHWC > alexnet_results
EOF
	chmod +x "${script}"
}

function tensorflow_test() {
	info "Copy Resnet Tensorflow test"
	local pids=()
	local j=0
	for i in "${containers[@]}"; do
		$(sudo -E "${CTR_EXE}" t exec -d --exec-id "$(random_name)" "${i}" sh -c "${CMD_RESNET}")&
		pids["${j}"]=$!
		((j++))
	done

	# wait for all pids
	for pid in ${pids[*]}; do
		wait "${pid}"
	done

	info "All containers are running the workload..."

	for i in "${containers[@]}"; do
		check_file=$(sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "${RESNET_CMD_FILE}")
		retries="100"
		for j in $(seq 1 "${retries}"); do
			[ "${check_file}" -eq "1" ] && break
			sleep 1
		done
	done

	info "Copy Alexnet Tensorflow test"
	local pids=()
	local j=0
	for i in "${containers[@]}"; do
		$(sudo -E "${CTR_EXE}" t exec -d --exec-id "$(random_name)" "${i}" sh -c "${CMD_ALEXNET}")&
		pids["${j}"]=$!
		((j++))
	done

	# wait for all pids
	for pid in ${pids[*]}; do
		wait "${pid}"
	done

	for i in "${containers[@]}"; do
		check_file=$(sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "${CMD_FILE}")
		retries="300"
		for j in $(seq 1 "${retries}"); do
			[ "${check_file}" -eq "1" ] && break
			sleep 1
		done
	done

	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "cat resnet_results"  >> "${resnet_tensorflow_file}"
		sudo -E "${CTR_EXE}" t exec --exec-id "$(random_name)" "${i}" sh -c "cat alexnet_results"  >> "${alexnet_tensorflow_file}"
	done

	local resnet_results=$(cat "${resnet_tensorflow_file}" | grep "total images/sec" | cut -d ":" -f2 | sed -e 's/^[ \t]*//' | tr '\n' ',' | sed 's/.$//')
	local average_resnet=$(echo "${resnet_results}" | sed "s/,/+/g;s/.*/(&)\/$NUM_CONTAINERS/g" | bc -l)

	local json="$(cat << EOF
	{
		"Resnet": {
			"Result": ${resnet_results},
			"Average": ${average_resnet},
			"Units": "images/s"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"

	local alexnet_results=$(cat "${alexnet_tensorflow_file}" | grep "total images/sec" | cut -d ":" -f2 | sed -e 's/^[ \t]*//' | tr '\n' ',' | sed 's/.$//')
	local average_alexnet=$(echo "${alexnet_results}" | sed "s/,/+/g;s/.*/(&)\/$NUM_CONTAINERS/g" | bc -l)

	local json="$(cat << EOF
	{
		"AlexNet": {
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
	check_containers_are_up

	# Check that the requested number of containers are running
	local timeout_launch="10"
	check_containers_are_up & pid=$!
	(sleep "${timeout_launch}" && kill -HUP "${pid}") 2>/dev/null & pid_tout=$!

	if wait "${pid}" 2>/dev/null; then
		pkill -HUP -P "${pid_tout}"
		wait "${pid_tout}"
	else
		warn "Time out exceeded"
		return 1
	fi

	# Get the initial number of pids in a single container before the workload starts
	INITIAL_NUM_PIDS=$(sudo -E "${CTR_EXE}" t metrics "${containers[-1]}" | grep pids.current | grep pids.current | xargs | cut -d ' ' -f 2)
	((INITIAL_NUM_PIDS++))

	tensorflow_test

	metrics_json_save

	rm -rf "${src_dir}"

	clean_env_ctr
}
main "$@"
