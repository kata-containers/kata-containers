#!/bin/bash
# Copyright (c) 2017-2021 Intel Corporation
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
IMAGE='quay.io/prometheus/busybox:latest'

CMD='tail -f /dev/null'
NUM_CONTAINERS="$1"
WAIT_TIME="$2"
AUTO_MODE="$3"
TEST_NAME="memory footprint"
SMEM_BIN="smem"
KSM_ENABLE_FILE="/sys/kernel/mm/ksm/run"
MEM_TMP_FILE=$(mktemp meminfo.XXXXXXXXXX)
PS_TMP_FILE=$(mktemp psinfo.XXXXXXXXXX)

function remove_tmp_file() {
	rm -rf $MEM_TMP_FILE $PS_TMP_FILE
}

trap remove_tmp_file EXIT

# Show help about this script
help(){
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


get_runc_pss_memory(){
	ctr_runc_shim_path="/usr/local/bin/containerd-shim-runc-v2"
	get_pss_memory "$ctr_runc_shim_path"
}

get_runc_individual_memory() {
	runc_process_result=$(cat $MEM_TMP_FILE | tr "\n" " " | sed -e 's/\s$//g' | sed 's/ /, /g')

	# Verify runc process result
	if [ -z "$runc_process_result" ];then
		die "Runc process not found"
	fi

	read -r -a runc_values <<< "${runc_process_result}"

	metrics_json_start_array

	local json="$(cat << EOF
	{
		"runc individual results": [
			$(for ((i=0;i<${NUM_CONTAINERS[@]};++i)); do
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
get_pss_memory(){
	ps="$1"
	mem_amount=0
	count=0
	avg=0

	if [ -z "$ps" ]; then
		die "No argument to get_pss_memory()"
	fi

	# Save all the processes names
	# This will be help us to retrieve raw information
	echo $ps >> $PS_TMP_FILE

	data=$(sudo "$SMEM_BIN" --no-header -P "^$ps" -c "pss" | sed 's/[[:space:]]//g')

	# Save all the smem results
	# This will help us to retrieve raw information
	echo $data >> $MEM_TMP_FILE

	for i in $data;do
		if (( i > 0 ));then
			mem_amount=$(( i + mem_amount ))
			(( count++ ))
		fi
	done

	if (( $count > 0 ));then
		avg=$(bc -l <<< "scale=2; $mem_amount / $count")
	fi

	echo "$avg"
}

ppid() {
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
get_pss_memory_virtiofsd() {
	mem_amount=0
	count=0
	avg=0

	virtiofsd_path=${1:-}
	if [ -z "${virtiofsd_path}" ]; then
		die "virtiofsd_path not provided"
	fi

	echo "${virtiofsd_path}" >> $PS_TMP_FILE

	virtiofsd_pids=$(ps aux | grep [v]irtiofsd | awk '{print $2}')
	data=$(sudo smem --no-header -P "^${virtiofsd_path}" -c pid -c "pid pss")

	for p in ${virtiofsd_pids}; do
		parent_pid=$(ppid ${p})
		cmd="$(cat /proc/${p}/cmdline | tr -d '\0')"
		cmd_parent="$(cat /proc/${parent_pid}/cmdline | tr -d '\0')"
		if [ "${cmd}" != "${cmd_parent}" ]; then
			pss_parent=$(printf "%s" "${data}" | grep "\s^${p}" | awk '{print $2}')

			fork=$(pgrep -P ${p})

			pss_fork=$(printf "%s" "${data}" | grep "^\s*${fork}" | awk '{print $2}')
			pss_process=$((pss_fork + pss_parent))

			# Save all the smem results
			# This will help us to retrieve raw information
			echo "${pss_process}" >>$MEM_TMP_FILE

			if ((pss_process > 0)); then
				mem_amount=$((pss_process + mem_amount))
				((count++))
			fi
		fi
	done

	if (( $count > 0 ));then
		avg=$(bc -l <<< "scale=2; $mem_amount / $count")
	fi
	echo "${avg}"
}

get_individual_memory(){
	# Getting all the individual container information
	first_process_name=$(cat $PS_TMP_FILE | awk 'NR==1' | awk -F "/" '{print $NF}' | sed 's/[[:space:]]//g')
	first_process_result=$(cat $MEM_TMP_FILE | awk 'NR==1' | sed 's/ /, /g')

	second_process_name=$(cat $PS_TMP_FILE | awk 'NR==2' | awk -F "/" '{print $NF}' | sed 's/[[:space:]]//g')
	second_process_result=$(cat $MEM_TMP_FILE | awk 'NR==2' | sed 's/ /, /g')

	third_process_name=$(cat $PS_TMP_FILE | awk 'NR==3' | awk -F "/" '{print $NF}' | sed 's/[[:space:]]//g')
	third_process_result=$(cat $MEM_TMP_FILE | awk 'NR==3' | sed 's/ /, /g')

	read -r -a first_values <<< "${first_process_result}"
	read -r -a second_values <<< "${second_process_result}"
	read -r -a third_values <<< "${third_process_result}"

	metrics_json_start_array

	local json="$(cat << EOF
	{
		"$first_process_name memory": [
			$(for ((i=0;i<${NUM_CONTAINERS[@]};++i)); do
				[ -n "${first_values[i]}" ] &&
				printf '%s\n\t\t\t' "${first_values[i]}"
			done)
		],
		"$second_process_name memory": [
			$(for ((i=0;i<${NUM_CONTAINERS[@]};++i)); do
				[ -n "${second_values[i]}" ] &&
				printf '%s\n\t\t\t' "${second_values[i]}"
			done)
		],
		"$third_process_name memory": [
			$(for ((i=0;i<${NUM_CONTAINERS[@]};++i)); do
				[ -n "${third_values[i]}" ] &&
				printf '%s\n\t\t\t' "${third_values[i]}"
			done)
		]
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Raw results"
}

# Try to work out the 'average memory footprint' of a container.
get_docker_memory_usage(){
	hypervisor_mem=0
	virtiofsd_mem=0
	shim_mem=0
	memory_usage=0

	containers=()

	for ((i=1; i<= NUM_CONTAINERS; i++)); do
		containers+=($(random_name))
		${CTR_EXE} run --runtime "${CTR_RUNTIME}" -d ${IMAGE}  ${containers[-1]} ${CMD}
	done

	if [ "$AUTO_MODE" == "auto" ]; then
		if (( ksm_on != 1 )); then
			die "KSM not enabled, cannot use auto mode"
		fi

		echo "Entering KSM settle auto detect mode..."
		wait_ksm_settle $WAIT_TIME
	else
		# If KSM is enabled, then you normally want to sleep long enough to
		# let it do its work and for the numbers to 'settle'.
		echo "napping $WAIT_TIME s"
		sleep "$WAIT_TIME"
	fi

	metrics_json_start_array
	# Check the runtime in order in order to determine which process will
	# be measured about PSS
	if [ "$RUNTIME" == "runc" ]; then
		runc_workload_mem="$(get_runc_pss_memory)"
		memory_usage="$runc_workload_mem"

	local json="$(cat << EOF
	{
		"average": {
			"Result": $memory_usage,
			"Units" : "KB"
		},
		"runc": {
			"Result": $runc_workload_mem,
			"Units" : "KB"
		}
	}
EOF
)"

	else [ "$RUNTIME" == "kata-runtime" ] || [ "$RUNTIME" == "kata-qemu" ]
		# Get PSS memory of VM runtime components.
		# And check that the smem search has found the process - we get a "0"
		#  back if that procedure fails (such as if a process has changed its name
		#  or is not running when expected to be so)
		# As an added bonus - this script must be run as root.
		# Now if you do not have enough rights
		#  the smem failure to read the stats will also be trapped.

		hypervisor_mem="$(get_pss_memory "$HYPERVISOR_PATH")"
		if [ "$hypervisor_mem" == "0" ]; then
			die "Failed to find PSS for $HYPERVISOR_PATH"
		fi

		virtiofsd_mem="$(get_pss_memory_virtiofsd "$VIRTIOFSD_PATH")"
		if [ "$virtiofsd_mem" == "0" ]; then
			echo >&2 "WARNING: Failed to find PSS for $VIRTIOFSD_PATH"
		fi
		shim_mem="$(get_pss_memory "$SHIM_PATH")"
		if [ "$shim_mem" == "0" ]; then
			die "Failed to find PSS for $SHIM_PATH"
		fi

		mem_usage="$(bc -l <<< "scale=2; $hypervisor_mem +$virtiofsd_mem + $shim_mem")"
		memory_usage="$mem_usage"

	local json="$(cat << EOF
	{
		"average": {
			"Result": $mem_usage,
			"Units" : "KB"
		},
		"qemus": {
			"Result": $hypervisor_mem,
			"Units" : "KB"
		},
		"virtiofsds": {
			"Result": $virtiofsd_mem,
			"Units" : "KB"
		},
		"shims": {
			"Result": $shim_mem,
			"Units" : "KB"
		}
	}
EOF
)"
	fi

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"

	clean_env_ctr
}

save_config(){
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"containers": $NUM_CONTAINERS,
		"ksm": $ksm_on,
		"auto": "$AUTO_MODE",
		"waittime": $WAIT_TIME,
		"image": "$IMAGE",
		"command": "$CMD"
	}
EOF

)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}

main(){
	# Verify enough arguments
	if [ $# != 2 ] && [ $# != 3 ];then
		echo >&2 "error: Not enough arguments [$@]"
		help
		exit 1
	fi

	#Check for KSM before reporting test name, as it can modify it
	check_for_ksm

	init_env

	check_cmds "${SMEM_BIN}" bc
	check_images "$IMAGE"

	if [ "${CTR_RUNTIME}" == "io.containerd.kata.v2" ]; then
		export RUNTIME="kata-runtime"
        elif [ "${CTR_RUNTIME}" == "io.containerd.runc.v2" ]; then
		export RUNTIME="runc"
        else
		die "Unknown runtime ${CTR_RUNTIME}"
	fi

	metrics_json_init
	save_config
	get_docker_memory_usage

	if [ "$RUNTIME" == "runc" ]; then
		get_runc_individual_memory
	elif [ "$RUNTIME" == "kata-runtime" ]; then
		get_individual_memory
	fi

	metrics_json_save
}

main "$@"
