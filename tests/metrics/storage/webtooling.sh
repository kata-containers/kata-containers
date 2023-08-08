#!/bin/bash
#
# Copyright (c) 2020-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description of the test:
# This test runs the 'web tooling benchmark'
# https://github.com/v8/web-tooling-benchmark

set -o pipefail

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"
NUM_CONTAINERS="${1:-}"

IMAGE="docker.io/library/local-web-tooling:latest"
DOCKERFILE="${SCRIPT_PATH}/web-tooling-dockerfile/Dockerfile"

# Directory to run the test inside of the container
TESTDIR="${TESTDIR:-/testdir}"
file_path="/web-tooling-benchmark"
file_name="output"

# Directory where the webtooling results are stored
TMP_DIR=$(mktemp --tmpdir -d webtool.XXXXXXXXXX)

# Options to control the start of the workload using a trigger-file
dst_dir="/host"
src_dir=$(mktemp --tmpdir -d webtool.XXXXXXXXXX)
trigger_file="$RANDOM.txt"
guest_trigger_file="$dst_dir/$trigger_file"
host_trigger_file="$src_dir/$trigger_file"
start_script="webtooling_start.sh"

# CMD points to the script that starts the workload
CMD="$dst_dir/$start_script"
MOUNT_OPTIONS="type=bind,src=$src_dir,dst=$dst_dir,options=rbind:ro"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"

# This timeout is related with the amount of time that
# webtool benchmark needs to run inside the container
timeout=600
INITIAL_NUM_PIDS=1

cpu_period="100000"
cpu_quota="200000"

function remove_tmp_dir() {
	rm -rf "$TMP_DIR"
	rm -rf "$src_dir"
}

trap remove_tmp_dir EXIT

# Show help about this script
function help(){
cat << EOF
Usage: $0 <count>
   Description:
       <count> : Number of containers to run.
EOF
}

# script used to launch the workload
function create_start_script() {
	local script="${src_dir/$start_script}"
	rm -rf "${script}"

cat <<EOF >>"${script}"
#!/bin/bash
mkdir -p "${TESTDIR}"

until [ -f ${guest_trigger_file} ]; do
	sleep 1
done
pushd "${file_path}"
node dist/cli.js > "${file_name}"
EOF
	chmod +x "${script}"
}

function verify_task_is_completed_on_all_containers() {
	local containers=( $(sudo -E "${CTR_EXE}" c list -q) )
	local sleep_secs=10
	local max=$(bc <<<"${timeout} / ${sleep_secs}")
	local wip_list=()
	local count=1
	local sum=0
	local i=""

	while (( ${sum} < ${NUM_CONTAINERS} )); do

	    for i in "${containers[@]}"; do
		# Only check containers that have not completed the workload at this step
		num_pids=$(sudo -E "${CTR_EXE}" t metrics "${i}" | grep pids.current | grep pids.current | xargs | cut -d ' ' -f 2)

	        if [ "${num_pids}" -lt "${INITIAL_NUM_PIDS}" ]; then
                    ((sum++))
                else
                    wip_list+=("${i}")
                fi
            done

            # hold the list of containers that are still running the workload
            containers=(${wip_list[*]})
            wip_list=()

            info "loop ${count} of ${max}: sleeping for ${sleep_secs} seconds"
            sleep "${sleep_secs}"
            ((count++))
	done
}

function check_containers_are_up() {
	info "Verify that the containers are running"
	local containers_launched=0
	while (( $containers_launched < ${NUM_CONTAINERS} )); do
		containers_launched="$(sudo -E ${CTR_EXE} t list | grep -c "RUNNING")"
		sleep 1
	done
}

function save_config() {
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"containers": "${NUM_CONTAINERS}",
		"image": "${IMAGE}",
		"units": "runs/s"
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Config"
}

function main() {
	# Verify enough arguments
	if [ $# != 1 ]; then
		echo >&2 "error: Not enough arguments [$@]"
		help
		exit 1
	fi

	local i=0
	local containers=()
	local cmds=("docker")
	local not_started_count=$NUM_CONTAINERS

	restart_containerd_service
	# Check tools/commands dependencies
	init_env
	check_cmds "${cmds[@]}"
	check_ctr_images "$IMAGE" "$DOCKERFILE"
	metrics_json_init
	save_config
	create_start_script
	rm -rf "${host_trigger_file}"

	info "Creating ${NUM_CONTAINERS} containers"

	for ((i=1; i<= "${NUM_CONTAINERS}"; i++)); do
		containers+=($(random_name))
		# Web tool benchmark needs 2 cpus to run completely in its cpu utilization
		sudo -E "${CTR_EXE}" run -d --runtime "${CTR_RUNTIME}" --cpu-quota "${cpu_quota}" --cpu-period "${cpu_period}" --mount="${MOUNT_OPTIONS}" "${IMAGE}" "${containers[-1]}" sh -c "${PAYLOAD_ARGS}"
		((not_started_count--))
		info "${not_started_count} remaining containers"
	done

	# Check that the requested number of containers are running
	local timeout_launch="10"
	check_containers_are_up & pid=$!
	(sleep "${timeout_launch}" && kill -HUP ${pid}) 2>/dev/null & pid_tout=$!

	if wait $pid 2>/dev/null; then
		pkill -HUP -P "${pid_tout}"
		wait "${pid_tout}"
	else
		warn "Time out exceeded"
		return 1
	fi

	# Get the initial number of pids in a single container before the workload starts
        INITIAL_NUM_PIDS=$(sudo -E "${CTR_EXE}" t metrics "${containers[-1]}" | grep pids.current | grep pids.current | xargs | cut -d ' ' -f 2)
	((INITIAL_NUM_PIDS++))

	# Launch webtooling benchmark
	local pids=()
	local j=0
	for i in "${containers[@]}"; do
		$(sudo -E "${CTR_EXE}" t exec -d --exec-id "$(random_name)" "${i}" sh -c "${CMD}") &
		pids[${j}]=$!
		((j++))
	done

	# wait for all pids
	for pid in ${pids[*]}; do
	    wait "${pid}"
	done

	touch "${host_trigger_file}"
	info "All containers are running the workload..."

	# Verify that all containers have completed the assigned task
	verify_task_is_completed_on_all_containers & pid=$!
	(sleep "$timeout" && kill -HUP $pid) 2>/dev/null & pid_tout=$!
	if wait ${pid} 2>/dev/null; then
		pkill -HUP -P "${pid_tout}"
		wait "${pid_tout}"
	else
		warn "Time out exceeded"
		return 1
	fi

	RESULTS_CMD="cat ${file_path}/${file_name}"
	for i in "${containers[@]}"; do
		sudo -E "${CTR_EXE}" t exec --exec-id "${RANDOM}" "${i}" sh -c "${RESULTS_CMD}" >> "${TMP_DIR}/results"
	done

	# Save configuration
	metrics_json_start_array

	local output=$(cat "${TMP_DIR}/results")
	local cut_results="cut -d':' -f2 | sed -e 's/^[ \t]*//'| cut -d ' ' -f1 | tr '\n' ',' | sed 's/.$//'"

	local acorn=$(echo "${output}" | grep -w "acorn" | eval "${cut_results}")
	local babel=$(echo "${output}" | grep -w "babel" | sed '/babel-minify/d' | eval "${cut_results}")
	local babel_minify=$(echo "${output}" | grep -w "babel-minify" | eval "${cut_results}")
	local babylon=$(echo "${output}" | grep -w "babylon" | eval "${cut_results}")
	local buble=$(echo "${output}" | grep -w "buble" | eval "${cut_results}")
	local chai=$(echo "${output}" | grep -w "chai" | eval "${cut_results}")
	local coffeescript=$(echo "${output}" | grep -w "coffeescript" | eval "${cut_results}")
	local espree=$(echo "${output}" | grep -w "espree" | eval "${cut_results}")
	local esprima=$(echo "${output}" | grep -w "esprima" | eval "${cut_results}")
	local jshint=$(echo "${output}" | grep -w "jshint" | eval "${cut_results}")
	local lebab=$(echo "${output}" | grep -w "lebab" | eval "${cut_results}")
	local postcss=$(echo "${output}" | grep -w "postcss" | eval "${cut_results}")
	local prepack=$(echo "${output}" | grep -w "prepack" | eval "${cut_results}")
	local prettier=$(echo "${output}" | grep -w "prettier" | eval "${cut_results}")
	local source_map=$(echo "${output}" | grep -w "source-map" | eval "${cut_results}")
	local terser=$(echo "${output}" | grep -w "terser" | eval "${cut_results}")
	local typescript=$(echo "${output}" | grep -w "typescript" | eval "${cut_results}")
	local uglify_js=$(echo "${output}" | grep -w "uglify-js" | eval "${cut_results}")
	local geometric_mean=$(echo "${output}" | grep -w "Geometric" | eval "${cut_results}")
	local average_tps=$(echo "${geometric_mean}" | sed "s/,/+/g;s/.*/(&)\/$NUM_CONTAINERS/g" | bc -l)
	local tps=$(echo "${average_tps}*${NUM_CONTAINERS}" | bc -l)

	local json="$(cat << EOF
	{
		"Acorn" : "${acorn}",
		"Babel" : "${babel}",
		"Babel minify" : "${babel_minify}",
		"Babylon" : "${babylon}",
		"Buble" : "${buble}",
		"Chai" : "${chai}",
		"Coffeescript" : "${coffeescript}",
		"Espree" : "${espree}",
		"Esprima" : "${esprima}",
		"Jshint" : "${jshint}",
		"Lebab" : "${lebab}",
		"Postcss" : "${postcss}",
		"Prepack" : "${prepack}",
		"Prettier" : "${prettier}",
		"Source map" : "${source_map}",
		"Terser" : "${terser}",
		"Typescript" : "${typescript}",
		"Uglify js" : "${uglify_js}",
		"Geometric mean" : "${geometric_mean}",
		"Average TPS" : "${average_tps}",
		"TPS" : "${tps}"
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Results"
	metrics_json_save
	clean_env_ctr
}

main "$@"
