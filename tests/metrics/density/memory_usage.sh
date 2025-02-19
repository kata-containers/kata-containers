#!/bin/bash
# Copyright (c) 2017-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#  Description of the test:
#  This test launches a number of containers in idle mode,
#  It will then sleep for a configurable period of time to allow
#  any memory optimisations to 'settle, and then checks the
#  amount of memory used by all the containers to come up with
#  an average (using the PSS measurements)
#  This test uses smem tool to get the memory used.

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

# Busybox image: Choose a small workload image, this is
# in order to measure the runtime footprint, not the workload
# footprint.
IMAGE="quay.io/prometheus/busybox:latest"

CMD='tail -f /dev/null'
NUM_CONTAINERS="$1"
WAIT_TIME="$2"
AUTO_MODE="$3"
TEST_NAME="memory footprint"
SMEM_BIN="smem"
KSM_ENABLE_FILE="/sys/kernel/mm/ksm/run"
MEM_TMP_FILE=$(mktemp meminfo.XXXXXXXXXX)
PS_TMP_FILE=$(mktemp psinfo.XXXXXXXXXX)
SKIP_VIRTIO_FS=0

# Variables used to collect memory footprint
global_hypervisor_mem=0
global_virtiofsd_mem=0
global_shim_mem=0

function remove_tmp_file() {
	rm -rf "${MEM_TMP_FILE}" "${PS_TMP_FILE}"
	clean_env_ctr
}

trap remove_tmp_file EXIT

# Show help about this script
function help(){
cat << EOF

Usage: $0 <count> <wait_time> [auto]

   Description:
        <count>      : Number of containers to run.
        <wait_time>  : Time in seconds to wait before taking
                       metrics.
        [auto]       : Optional 'auto KSM settle' mode
                       waits for ksm pages_shared to settle down

EOF
}


function get_runc_pss_memory(){
	ctr_runc_shim_path="/usr/local/bin/containerd-shim-runc-v2"
	get_pss_memory "${ctr_runc_shim_path}"
}

function get_runc_individual_memory() {
	runc_process_result=$(cat "${MEM_TMP_FILE}" | tr "\n" " " | sed -e 's/\s$//g' | sed 's/ /, /g')

	# Verify runc process result
	if [ -z "${runc_process_result}" ];then
		die "Runc process not found"
	fi

	read -r -a runc_values <<< "${runc_process_result}"

	metrics_json_start_array

	local json="$(cat << EOF
	{
		"runc individual results": [
			$(for ((i=0;i<"${NUM_CONTAINERS[@]}";++i)); do
				printf '%s\n\t\t\t' "${runc_values[i]}"
			done)
		]
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Raw results"
}

# This function measures the PSS average
# memory of a process.
function get_pss_memory(){
	local ps="${1}"
	local shim_result_on="${2:-0}"
	local mem_amount=0
	local count=0
	local avg=0

	[ -z "${ps}" ] && die "No argument to get_pss_memory()"

	ps="$(readlink -f ${ps})"

	# Save all the processes names
	# This will be help us to retrieve raw information
	echo "${ps}" >> "${PS_TMP_FILE}"

	data="$(sudo "${SMEM_BIN}" --no-header -P "^${ps}" -c "pss" | sed 's/[[:space:]]//g' | tr '\n' ' ' | sed 's/[[:blank:]]*$//')"

	# Save all the smem results
	# This will help us to retrieve raw information
	echo "${data}" >> "${MEM_TMP_FILE}"

	arrData=(${data})

	for i in "${arrData[@]}"; do
		if [ "${i}" -gt 0 ]; then
			let "mem_amount+=i"
			let "count+=1"
			[ "${count}" -eq "${NUM_CONTAINERS}" ] && break
		fi
	done

	[ "${count}" -eq 0 ] && die "No pss memory was measured for PID: ${ps}"

	avg=$(bc -l <<< "scale=2; ${mem_amount} / ${count}")

	if [ "${shim_result_on}" -eq "1" ]; then
		global_shim_mem="${avg}"
	else
		global_hypervisor_mem="${avg}"
	fi
}

function ppid() {
	local pid
	pid=$(ps -p "${1:-nopid}" -o ppid=)
	echo "${pid//[[:blank:]]/}"
}

# This function measures the PSS average
# memory of virtiofsd.
# It is a special case of get_pss_memory,
# virtiofsd forks itself so, smem sees the process
# two times, this function sum both pss values:
# pss_virtiofsd=pss_fork + pss_parent
function get_pss_memory_virtiofsd() {
	mem_amount=0
	count=0
	avg=0
	virtiofsd_path="${1:-}"

	if [ $(ps aux | grep -c '[v]irtiofsd') -eq 0 ]; then
		SKIP_VIRTIO_FS=1
		return
	fi

	[ -z "${virtiofsd_path}" ] && die "virtiofs path not provided"

	echo "${virtiofsd_path}" >> "${PS_TMP_FILE}"

	virtiofsd_pids="$(ps aux | grep '[v]irtiofsd' | awk '{print $2}' | head -1)"

	data="$(sudo smem --no-header -P "^${virtiofsd_path}" -c "pid pss")"

	for p in "${virtiofsd_pids}"; do
		echo "get_pss_memory_virtiofsd: p=${p}"
		parent_pid=$(ppid "${p}")
		cmd="$(cat /proc/${p}/cmdline | tr -d '\0')"
		cmd_parent="$(cat /proc/${parent_pid}/cmdline | tr -d '\0')"
		if [ "${cmd}" != "${cmd_parent}" ]; then
			pss_parent=$(printf "%s" "${data}" | grep "\s^${p}" | awk '{print $2}')

			fork=$(pgrep -P "${p}")

			pss_fork=$(printf "%s" "${data}" | grep "^\s*${fork}" | awk '{print $2}')
			pss_process=$((pss_fork + pss_parent))

			# Save all the smem results
			# This will help us to retrieve raw information
			echo "${pss_process}" >>"${MEM_TMP_FILE}"

			if ((pss_process > 0)); then
				mem_amount=$((pss_process + mem_amount))
				let "count+=1"
			fi
		fi
	done

	[ "${count}" -gt 0 ] && global_virtiofsd_mem=$(bc -l <<< "scale=2; ${mem_amount} / ${count}")
}

function get_individual_memory(){
	# Getting all the individual container information
	first_process_name=$(cat "${PS_TMP_FILE}" | awk 'NR==1' | awk -F "/" '{print $NF}' | sed 's/[[:space:]]//g')
	first_process_result=$(cat "${MEM_TMP_FILE}" | awk 'NR==1' | sed 's/ /, /g')

	second_process_name=$(cat "${PS_TMP_FILE}" | awk 'NR==2' | awk -F "/" '{print $NF}' | sed 's/[[:space:]]//g')
	second_process_result=$(cat "${MEM_TMP_FILE}" | awk 'NR==2' | sed 's/ /, /g')

	third_process_name=$(cat "${PS_TMP_FILE}" | awk 'NR==3' | awk -F "/" '{print $NF}' | sed 's/[[:space:]]//g')
	third_process_result=$(cat "${MEM_TMP_FILE}" | awk 'NR==3' | sed 's/ /, /g')

	read -r -a first_values <<< "${first_process_result}"
	read -r -a second_values <<< "${second_process_result}"
	read -r -a third_values <<< "${third_process_result}"

	declare -a fv_array
	declare -a sv_array
	declare -a tv_array

	# remove null values from arrays of results
	for ((i=0;i<"${NUM_CONTAINERS}";++i)); do
		[ -n "${first_values[i]}" ] && fv_array+=( "${first_values[i]}" )
		[ -n "${second_values[i]}" ] && sv_array+=( "${second_values[i]}" )
		[ -n "${third_values[i]}" ] && tv_array+=( "${third_values[i]}" )
	done

	# remove trailing commas
	fv_array[-1]="$(sed -r 's/,*$//g' <<< "${fv_array[-1]}")"
	sv_array[-1]="$(sed -r 's/,*$//g' <<< "${sv_array[-1]}")"

	[ "${SKIP_VIRTIO_FS}" -eq 0 ] && tv_array[-1]="$(sed -r 's/,*$//g' <<< "${tv_array[-1]}")"

	metrics_json_start_array

	local json="$(cat << EOF
	{
		"${first_process_name} memory": [
			$(for i in "${fv_array[@]}"; do
				printf '\n\t\t\t%s' "${i}"
			done)
		],
		"${second_process_name} memory": [
			$(for i in "${sv_array[@]}"; do
				printf '\n\t\t\t%s' "${i}"
			done)
		],
		"${third_process_name} memory": [
			$(for i in "${tv_array[@]}"; do
				printf '\n\t\t\t%s' "${i}"
			done)
		]
	}
EOF
)"
	metrics_json_add_array_element "${json}"
	metrics_json_end_array "Raw results"
}

# Try to work out the 'average memory footprint' of a container.
function get_memory_usage(){
	hypervisor_mem=0
	virtiofsd_mem=0
	shim_mem=0
	memory_usage=0

	containers=()

	info "Creating ${NUM_CONTAINERS} containers"
	for ((i=1; i<="${NUM_CONTAINERS}"; i++)); do
		containers+=($(random_name))
		sudo "${CTR_EXE}" run --runtime "${CTR_RUNTIME}" -d "${IMAGE}" "${containers[-1]}" sh -c "${CMD}"
	done

	if [ "${AUTO_MODE}" == "auto" ]; then
		if (( ksm_on != 1 )); then
			die "KSM not enabled, cannot use auto mode"
		fi

		echo "Entering KSM settle auto detect mode..."
		wait_ksm_settle "${WAIT_TIME}"
	else
		# If KSM is enabled, then you normally want to sleep long enough to
		# let it do its work and for the numbers to 'settle'.
		echo "napping ${WAIT_TIME} s"
		sleep "${WAIT_TIME}"
	fi

	metrics_json_start_array
	# Check the runtime in order in order to determine which process will
	# be measured about PSS
	if [ "${CTR_RUNTIME}" == "io.containerd.runc.v2" ]; then
		runc_workload_mem="$(get_runc_pss_memory)"
		memory_usage="${runc_workload_mem}"

		local json="$(cat << EOF
	{
		"average": {
			"Result": ${memory_usage},
			"Units" : "KB"
		},
		"runc": {
			"Result": ${runc_workload_mem},
			"Units" : "KB"
		}
	}
EOF
)"
	else [ "${CTR_RUNTIME}" == "io.containerd.kata.v2" ]
		# Get PSS memory of VM runtime components.
		# And check that the smem search has found the process - we get a "0"
		#  back if that procedure fails (such as if a process has changed its name
		#  or is not running when expected to be so)
		# As an added bonus - this script must be run as root.
		# Now if you do not have enough rights
		#  the smem failure to read the stats will also be trapped.
		get_pss_memory "${HYPERVISOR_PATH}"

		if [ "${global_hypervisor_mem}" == "0" ]; then
			die "Failed to find PSS for ${HYPERVISOR_PATH}"
		fi

		get_pss_memory_virtiofsd "${VIRTIOFSD_PATH}"

		if [ "${global_virtiofsd_mem}" == "0" ]; then
			echo >&2 "WARNING: Failed to find PSS for ${VIRTIOFSD_PATH}"
		fi
		get_pss_memory "${SHIM_PATH}" 1

		if [ "${global_shim_mem}" == "0" ]; then
			die "Failed to find PSS for ${SHIM_PATH}"
		fi
		mem_usage="$(bc -l <<< "scale=2; ${global_hypervisor_mem} + ${global_virtiofsd_mem} + ${global_shim_mem}")"

		local json="$(cat << EOF
	{
		"average": {
			"Result": ${mem_usage},
			"Units" : "KB"
		},
		"qemus": {
			"Result": ${global_hypervisor_mem},
			"Units" : "KB"
		},
		"virtiofsds": {
			"Result": ${global_virtiofsd_mem},
			"Units" : "KB"
		},
		"shims": {
			"Result": ${global_shim_mem},
			"Units" : "KB"
		}
	}
EOF
)"
	fi

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
}

function save_config(){
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"containers": "${NUM_CONTAINERS}",
		"ksm": "${ksm_on}",
		"auto": "${AUTO_MODE}",
		"waittime": "${WAIT_TIME}",
		"image": "${IMAGE}",
		"command": "${CMD}"
	}
EOF

)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}

function main(){
	# Verify enough arguments
	if [ $# != 2 ] && [ $# != 3 ];then
		help
		die "Not enough arguments [$@]"
	fi

	if [ "${CTR_RUNTIME}" != "io.containerd.runc.v2" ] && [ "${CTR_RUNTIME}" != "io.containerd.kata.v2" ]; then
		die "Unknown runtime: ${CTR_RUNTIME}."
	fi

	#Check for KSM before reporting test name, as it can modify it
	check_for_ksm
	check_cmds "${SMEM_BIN}" bc
	init_env
	check_images "${IMAGE}"

	metrics_json_init
	save_config
	get_memory_usage

	if [ "${CTR_RUNTIME}" == "io.containerd.kata.v2" ]; then
		get_individual_memory
        elif [ "${CTR_RUNTIME}" == "io.containerd.runc.v2" ]; then
		get_runc_individual_memory
	fi

	info "memory usage test completed"
	metrics_json_save
}

main "$@"
