#!/bin/bash
# Copyright (c) 2017-2018 Intel Corporation
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
IMAGE='busybox'

CMD='sh'
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

# This function measures the PSS average
# memory about the child process of each
# docker-containerd-shim instance in the
# system.
get_runc_pss_memory(){
        avg=0
        docker_shim="docker-containerd-shim"
        mem_amount=0
        count=0

        shim_instances=$(pgrep  -f "^$docker_shim")
        for shim in $shim_instances; do
                child_pid="$(pgrep -P $shim)"
                child_mem=$(sudo "$SMEM_BIN" -c "pid pss" | \
                                awk "/^$child_pid / {print \$2}")

		# Getting all individual results
		echo $child_mem >> $MEM_TMP_FILE

                if (( $child_mem > 0 ));then
                        mem_amount=$(( $child_mem + $mem_amount ))
                        (( count++ ))
                fi
        done

        # Calculate average
        if (( $count > 0 )); then
                avg=$(bc -l <<< "scale=2; $mem_amount / $count")
        fi

        echo "$avg"
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
				printf '%s\n\t\t\t' "${first_values[i]}"
			done)
		],
		"$second_process_name memory": [
			$(for ((i=0;i<${NUM_CONTAINERS[@]};++i)); do
				printf '%s\n\t\t\t' "${second_values[i]}"
			done)
		],
		"$third_process_name memory": [
			$(for ((i=0;i<${NUM_CONTAINERS[@]};++i)); do
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
	proxy_mem=0
	proxy_mem=0
	memory_usage=0

	containers=()

	for ((i=1; i<= NUM_CONTAINERS; i++)); do
		containers+=($(random_name))
		${DOCKER_EXE} run --runtime "$RUNTIME" --name ${containers[-1]} -tid $IMAGE $CMD
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

	elif [ "$RUNTIME" == "kata-qemu" ] || [ "$RUNTIME" == "kata-fc" ] || [ "$RUNTIME" == "kata-runtime" ]; then
		# Get PSS memory of VM runtime components.
		# And check that the smem search has found the process - we get a "0"
		#  back if that procedure fails (such as if a process has changed its name
		#  or is not running when expected to be so)
		# As an added bonus - this script must be run as root (or at least as
		#  a user with enough rights to allow smem to read the smap stats for
		#  the docker owned processes). Now if you do not have enough rights
		#  the smem failure to read the stats will also be trapped.

		hypervisor_mem="$(get_pss_memory "$HYPERVISOR_PATH")"
		if [ "$hypervisor_mem" == "0" ]; then
			die "Failed to find PSS for $HYPERVISOR_PATH"
		fi

		virtiofsd_mem="$(get_pss_memory "$VIRTIOFSD_PATH")"
		if [ "$virtiofsd_mem" == "0" ]; then
			echo >&2 "WARNING: Failed to find PSS for $VIRTIOFSD_PATH"
		fi
		shim_mem="$(get_pss_memory "$SHIM_PATH")"
		if [ "$shim_mem" == "0" ]; then
			die "Failed to find PSS for $SHIM_PATH"
		fi

		# Some runtimes do not have a proxy, so just set it to 0 space...
		if [ "$PROXY_PATH" != "" ]; then
			proxy_mem="$(get_pss_memory "$PROXY_PATH")"
			if [ "$proxy_mem" == "0" ]; then
				die "Failed to find PSS for $PROXY_PATH"
			fi
		else
			proxy_mem=0
		fi

		proxy_mem="$(bc -l <<< "scale=2; $proxy_mem / $NUM_CONTAINERS")"
		mem_usage="$(bc -l <<< "scale=2; $hypervisor_mem +$virtiofsd_mem + $shim_mem + $proxy_mem")"
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
		},
		"proxys": {
			"Result": $proxy_mem,
			"Units" : "KB"
		}
	}
EOF
)"
	else
		die "Unknown runtime: $RUNTIME"
	fi

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"

	docker rm -f ${containers[@]}
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

	metrics_json_init
	save_config
	get_docker_memory_usage

	if [ "$RUNTIME" == "runc" ]; then
		get_runc_individual_memory
	elif [ "$RUNTIME" == "cor" ] || [ "$RUNTIME" == "cc-runtime" ] || [ "$RUNTIME" == "kata-runtime" ]; then
		get_individual_memory
	fi

	metrics_json_save
}

main "$@"
