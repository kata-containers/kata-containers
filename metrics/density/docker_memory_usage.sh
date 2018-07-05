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

# See if KSM is enabled.
# If so, ammend the test name to reflect that
check_for_ksm(){
	if [ ! -f ${KSM_ENABLE_FILE} ]; then
		return
	fi

	ksm_on=$(< ${KSM_ENABLE_FILE})

	if [ $ksm_on == "1" ]; then
		TEST_NAME="${TEST_NAME} ksm"
	fi
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

	#FIXME - it would be nice to spit out this data into
	# the JSON file as well.
        shim_instances=$(pgrep  -f "^$docker_shim")
        for shim in $shim_instances; do
                child_pid="$(pgrep -P $shim)"
                child_mem=$(sudo "$SMEM_BIN" -c "pid pss" | \
                                grep "$child_pid" | awk '{print $2}')
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

	# FIXME - it would be nice to spit this data out into the
	# JSON file as well.
	# See issue [232](https://github.com/kata-containers/tests/issues/232)
	data=$(sudo "$SMEM_BIN" --no-header -P "^$ps" -c "pss")
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

# Wait for KSM to settle down, or timeout waiting
# The basic algorithm is to look at the pages_shared value
# at the end of every 'full scan', and if the value
# has changed very little, then we are done (because we presume
# a full scan has managed to do few new merges)
#
# arg1 - timeout in seconds
wait_ksm_settle(){
	local t pcnt
	local oldscan=-1 newscan
	local oldpages=-1 newpages

	oldscan=$(cat /sys/kernel/mm/ksm/full_scans)

	# Go around the loop until either we see a small % change
	# between two full_scans, or we timeout
	for ((t=0; t<$1; t++)); do

		newscan=$(cat /sys/kernel/mm/ksm/full_scans)
		if (( newscan != oldscan )); then
			echo -e "\nnew full_scan ($oldscan to $newscan)"

			newpages=$(cat /sys/kernel/mm/ksm/pages_shared)
			# Do we have a previous scan to compare with
			echo "check pages $oldpages to $newpages"
			if (( oldpages != -1 )); then
				# avoid divide by zero problems
				if (( $oldpages > 0 )); then
					pcnt=$(( 100 - ((newpages * 100) / oldpages) ))
					# abs()
					pcnt=$(( $pcnt * -1 ))

					echo "$oldpages to $newpages is ${pcnt}%"

					if (( $pcnt <= 5 )); then
						echo "KSM stabilised at ${t}s"
						return
					fi
				else
					echo "$oldpages KSM pages... waiting"
				fi
			fi
			oldscan=$newscan
			oldpages=$newpages
		else
			echo -n "."
		fi
		sleep 1
	done
	echo "Timed out after ${1}s waiting for KSM to settle"
}

# Try to work out the 'average memory footprint' of a container.
get_docker_memory_usage(){
	hypervisor_mem=0
	shim_mem=0
	proxy_mem=0
	proxy_mem=0
	memory_usage=0

	containers=()

	for ((i=1; i<= NUM_CONTAINERS; i++)); do
		containers+=($(random_name))
		${DOCKER_EXE} run --rm --runtime "$RUNTIME" --name ${containers[-1]} -tid $IMAGE $CMD
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

	elif [ "$RUNTIME" == "cor" ] || [ "$RUNTIME" == "cc-runtime" ] || [ "$RUNTIME" == "kata-runtime" ]; then
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

		shim_mem="$(get_pss_memory "$SHIM_PATH")"
		if [ "$shim_mem" == "0" ]; then
			die "Failed to find PSS for $SHIM_PATH"
		fi

		proxy_mem="$(get_pss_memory "$PROXY_PATH")"
		if [ "$proxy_mem" == "0" ]; then
			die "Failed to find PSS for $PROXY_PATH"
		fi

		proxy_mem="$(bc -l <<< "scale=2; $proxy_mem / $NUM_CONTAINERS")"
		mem_usage="$(bc -l <<< "scale=2; $hypervisor_mem + $shim_mem + $proxy_mem")"
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

	# FIXME - temporary workaround for https://github.com/kata-containers/runtime/issues/406
	# When that is fixed, we can go back to a hard 'docker rm -f *' with no sleep.
	# Or, in fact we can leave the '--rm' on the docker run, and just change this
	# to a 'docker stop *'.
	for c in ${containers[@]}; do
		docker stop $c
		sleep 3
	done
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
	metrics_json_save
}

main "$@"
