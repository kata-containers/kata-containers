#!/bin/bash
# Copyright (c) 2017-2018, 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# A script to gather memory 'footprint' information as we launch more
# and more containers
#
# The script gathers information about both user and kernel space consumption
# Output is into a .json file, named using some of the config component names
# (such as footprint-busybox.json)

# Pull in some common, useful, items
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

# Note that all vars that can be set from outside the script (that is,
# passed in the ENV), use the ':-' setting to allow being over-ridden

# Default sleep, in seconds, to let containers come up and finish their
# initialisation before we take the measures. Some of the larger
# containers can take a number of seconds to get running.
PAYLOAD_SLEEP="${PAYLOAD_SLEEP:-10}"

# How long, in seconds, do we wait for KSM to 'settle down', before we
# timeout and just continue anyway.
KSM_WAIT_TIME="${KSM_WAIT_TIME:-300}"

# How long, in seconds, do we poll for ctr to complete launching all the
# containers?
CTR_POLL_TIMEOUT="${CTR_POLL_TIMEOUT:-300}"

# How many containers do we launch in parallel before taking the PAYLOAD_SLEEP
# nap
PARALLELISM="${PARALLELISM:-10}"

### The default config - run a small busybox image
# Define what we will be running (app under test)
#  Default is we run busybox, as a 'small' workload
PAYLOAD="${PAYLOAD:-quay.io/prometheus/busybox:latest}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"

###
# which RUNTIME we use is picked up from the env in
# common.bash. You can over-ride by setting RUNTIME in your env

###
# Define the cutoff checks for when we stop running the test
  # Run up to this many containers
NUM_CONTAINERS="${NUM_CONTAINERS:-100}"
  # Run until we have consumed this much memory (from MemFree)
MAX_MEMORY_CONSUMED="${MAX_MEMORY_CONSUMED:-256*1024*1024*1024}"
  # Run until we have this much MemFree left
MIN_MEMORY_FREE="${MIN_MEMORY_FREE:-2*1024*1024*1024}"

# Tools we need to have installed in order to operate
REQUIRED_COMMANDS="smem awk"

# If we 'dump' the system caches before we measure then we get less
# noise in the results - they show more what our un-reclaimable footprint is
DUMP_CACHES="${DUMP_CACHES:-1}"

# Affects the name of the file to store the results in
TEST_NAME="${TEST_NAME:-fast-footprint-busybox}"

############# end of configurable items ###################

# vars to remember where we started so we can calc diffs
base_mem_avail=0
base_mem_free=0

# dump the kernel caches, so we get a more precise (or just different)
# view of what our footprint really is.
function dump_caches() {
	sudo bash -c "echo 3 > /proc/sys/vm/drop_caches"
}

function init() {
	restart_containerd_service

	check_cmds $REQUIRED_COMMANDS
	sudo -E "${CTR_EXE}" image pull "$PAYLOAD"

	# Modify the test name if running with KSM enabled
	check_for_ksm

	# Use the common init func to get to a known state
	init_env

	# Prepare to start storing results
	metrics_json_init

	# Store up baseline measures
	base_mem_avail=$(free -b | head -2 | tail -1 | awk '{print $7}')
	base_mem_free=$(get_memfree)

	# Store our configuration for this run
	save_config
}

save_config(){
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"testname": "${TEST_NAME}",
		"payload": "${PAYLOAD}",
		"payload_args": "${PAYLOAD_ARGS}",
		"payload_sleep": ${PAYLOAD_SLEEP},
		"ksm_settle_time": ${KSM_WAIT_TIME},
		"num_containers": ${NUM_CONTAINERS},
		"parallelism": ${PARALLELISM},
		"max_memory_consumed": "${MAX_MEMORY_CONSUMED}",
		"min_memory_free": "${MIN_MEMORY_FREE}",
		"dump_caches": "${DUMP_CACHES}"
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}

function cleanup() {
	# Finish storing the results
	metrics_json_save

	clean_env_ctr
}

# helper function to get USS of process in arg1
function get_proc_uss() {
	item=$(sudo smem -t -P "^$1" | tail -1 | awk '{print $4}')
	((item*=1024))
	echo $item
}

# helper function to get PSS of process in arg1
function get_proc_pss() {
	item=$(sudo smem -t -P "^$1" | tail -1 | awk '{print $5}')
	((item*=1024))
	echo $item
}

# Get the PSS for the whole of userspace (all processes)
#  This allows us to see if we had any impact on the rest of the system, for instance
#  dockerd grows as we launch containers, so we should account for that in our total
#  memory breakdown
function grab_all_pss() {
	item=$(sudo smem -t | tail -1 | awk '{print $5}')
	((item*=1024))

	local json="$(cat << EOF
		"all_pss": {
			"pss": $item,
			"Units": "KB"
		}
EOF
)"

	metrics_json_add_array_fragment "$json"
}

function grab_user_smem() {
	# userspace
	item=$(sudo smem -w | head -5 | tail -1 | awk '{print $3}')
	((item*=1024))

	local json="$(cat << EOF
		"user_smem": {
			"userspace": $item,
			"Units": "KB"
		}
EOF
)"

	metrics_json_add_array_fragment "$json"
}

function grab_slab() {
	# Grabbing slab total from meminfo is easier than doing the math
	# on slabinfo
	item=$(fgrep "Slab:" /proc/meminfo | awk '{print $2}')
	((item*=1024))

	local json="$(cat << EOF
		"slab": {
			"slab": $item,
			"Units": "KB"
		}
EOF
)"

	metrics_json_add_array_fragment "$json"
}

function get_memfree() {
	mem_free=$(sudo smem -w | head -6 | tail -1 | awk '{print $4}')
	((mem_free*=1024))
	echo $mem_free
}

function grab_system() {

	# avail memory, from 'free'
	local avail=$(free -b | head -2 | tail -1 | awk '{print $7}')
	local avail_decr=$((base_mem_avail-avail))

	# cached memory, from 'free'
	local cached=$(free -b | head -2 | tail -1 | awk '{print $6}')

	# free memory from smem
	local smem_free=$(get_memfree)
	local free_decr=$((base_mem_free-item))

	# Anon pages
	local anon=$(fgrep "AnonPages:" /proc/meminfo | awk '{print $2}')
	((anon*=1024))

	# Mapped pages
	local mapped=$(egrep "^Mapped:" /proc/meminfo | awk '{print $2}')
	((mapped*=1024))

	# Cached
	local meminfo_cached=$(grep "^Cached:" /proc/meminfo | awk '{print $2}')
	((meminfo_cached*=1024))

	local json="$(cat << EOF
		"system": {
			"avail": $avail,
			"avail_decr": $avail_decr,
			"cached": $cached,
			"smem_free": $smem_free,
			"free_decr": $free_decr,
			"anon": $anon,
			"mapped": $mapped,
			"meminfo_cached": $meminfo_cached,
			"Units": "KB"
		}
EOF
)"

	metrics_json_add_array_fragment "$json"
}

function grab_stats() {
	# If configured, dump the caches so we get a more stable
	# view of what our static footprint really is
	if [[ "$DUMP_CACHES" ]] ; then
		dump_caches
	fi

	# user space data
		# PSS taken all userspace
	grab_all_pss
		# user as reported by smem
	grab_user_smem

	# System overview data
		# System free and cached
	grab_system

	# kernel data
		# The 'total kernel space taken' we can work out as:
		# ktotal = ((free-avail)-user)
		# So, we don't grab that number from smem, as that is what it does
		# internally anyhow.
		# Still try to grab any finer kernel details that we can though

		# totals from slabinfo
	grab_slab

	metrics_json_close_array_element
}

function check_limits() {
	mem_free=$(get_memfree)
	if ((mem_free <= MIN_MEMORY_FREE)); then
		echo 1
		return
	fi

	mem_consumed=$((base_mem_avail-mem_free))
	if ((mem_consumed >= MAX_MEMORY_CONSUMED)); then
		echo 1
		return
	fi

	echo 0
}

launch_containers() {
	local parloops leftovers

	(( parloops=${NUM_CONTAINERS}/${PARALLELISM} ))
	(( leftovers=${NUM_CONTAINERS} - (${parloops}*${PARALLELISM}) ))

	echo "Launching ${parloops}x${PARALLELISM} containers + ${leftovers} etras"

	containers=()

	local iter n
	for iter in $(seq 1 $parloops); do
		echo "Launch iteration ${iter}"
		for n in $(seq 1 $PARALLELISM); do
			containers+=($(random_name))
			sudo -E "${CTR_EXE}" run -d --runtime=$CTR_RUNTIME $PAYLOAD ${containers[-1]} sh -c $PAYLOAD_ARGS &
		done

		if [[ $PAYLOAD_SLEEP ]]; then
			sleep $PAYLOAD_SLEEP
		fi

		# check if we have hit one of our limits and need to wrap up the tests
		if (($(check_limits))); then
			echo "Ran out of resources, check_limits failed"
			return
		fi
	done

	for n in $(seq 1 $leftovers); do
		containers+=($(random_name))
		sudo -E "${CTR_EXE}" run -d --runtime=$CTR_RUNTIME $PAYLOAD ${containers[-1]} sh -c $PAYLOAD_ARGS &
	done
}

wait_containers() {
	local t numcontainers
	# nap 3s between checks
	local step=3

	for ((t=0; t<${CTR_POLL_TIMEOUT}; t+=step)); do

		numcontainers=$(sudo -E "${CTR_EXE}" c list -q | wc -l)

		if (( numcontainers >=  ${NUM_CONTAINERS} )); then
			echo "All containers now launched (${t}s)"
				return
		else
			echo "Waiting for containers to launch (${numcontainers} at ${t}s)"
		fi
		sleep ${step}
	done

	echo "Timed out waiting for containers to launch (${t}s)"
	cleanup
	die "Timed out waiting for containers to launch (${t}s)"
}

function go() {
	# Init the json cycle for this save
	metrics_json_start_array

	# Grab the first set of stats before we run any containers.
	grab_stats

	launch_containers
	wait_containers

	if [ $ksm_on == "1" ]; then
		echo "Wating for KSM to settle..."
		wait_ksm_settle ${KSM_WAIT_TIME}
	fi

	grab_stats

	# Wrap up the results array
	metrics_json_end_array "Results"
}

function show_vars()
{
	echo -e "\nEvironment variables:"
	echo -e "\tName (default)"
	echo -e "\t\tDescription"
	echo -e "\tPAYLOAD (${PAYLOAD})"
	echo -e "\t\tThe ctr image to run"
	echo -e "\tPAYLOAD_ARGS (${PAYLOAD_ARGS})"
	echo -e "\t\tAny extra arguments passed into the docker 'run' command"
	echo -e "\tPAYLOAD_SLEEP (${PAYLOAD_SLEEP})"
	echo -e "\t\tSeconds to sleep between launch and measurement, to allow settling"
	echo -e "\tKSM_WAIT_TIME (${KSM_WAIT_TIME})"
	echo -e "\t\tSeconds to wait for KSM to settle before we take the final measure"
	echo -e "\tCTR_POLL_TIMEOUT (${CTR_POLL_TIMEOUT})"
	echo -e "\t\tSeconds to poll for ctr to finish launching containers"
	echo -e "\tPARALLELISM (${PARALLELISM})"
	echo -e "\t\tNumber of containers we launch in parallel"
	echo -e "\tNUM_CONTAINERS (${NUM_CONTAINERS})"
	echo -e "\t\tThe total number of containers to run"
	echo -e "\tMAX_MEMORY_CONSUMED (${MAX_MEMORY_CONSUMED})"
	echo -e "\t\tThe maximum amount of memory to be consumed before terminating"
	echo -e "\tMIN_MEMORY_FREE (${MIN_MEMORY_FREE})"
	echo -e "\t\tThe minimum amount of memory allowed to be free before terminating"
	echo -e "\tDUMP_CACHES (${DUMP_CACHES})"
	echo -e "\t\tA flag to note if the system caches should be dumped before capturing stats"
	echo -e "\tTEST_NAME (${TEST_NAME})"
	echo -e "\t\tCan be set to over-ride the default JSON results filename"

}

function help()
{
	usage=$(cat << EOF
Usage: $0 [-h] [options]
   Description:
	Launch a series of workloads and take memory metric measurements after
	each launch.
   Options:
        -h,    Help page.
EOF
)
	echo "$usage"
	show_vars
}

function main() {

	local OPTIND
	while getopts "h" opt;do
		case ${opt} in
		h)
		    help
		    exit 0;
		    ;;
		esac
	done
	shift $((OPTIND-1))

	init
	go
	cleanup
}

main "$@"
