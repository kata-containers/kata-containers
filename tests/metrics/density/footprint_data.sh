#!/bin/bash
# Copyright (c) 2017-2018 Intel Corporation
# 
# SPDX-License-Identifier: Apache-2.0
#
# A script to gather memory 'footprint' information as we launch more
# and more containers
# It allows configuration of a number of things:
# - which container workload we run
# - which container runtime we run with
# - when do we terminate the test (cutoff points)
#
# There are a number of things we may wish to add to this script later:
# - sanity check that the correct number of runtime components (qemu, shim etc.)
#  are running at all times
# - some post-processing scripts to generate stats and graphs
#
# The script gathers information about both user and kernel space consumption
# Output is into a .json file, named using some of the config component names
# (such as footprint-busybox.json)

# Pull in some common, useful, items
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

KSM_ENABLE_FILE="/sys/kernel/mm/ksm/run"

# Note that all vars that can be set from outside the script (that is,
# passed in the ENV), use the ':-' setting to allow being over-ridden

# Default sleep for 10s to let containers come up and finish their
# initialisation before we take the measures. Some of the larger
# containers can take a number of seconds to get running.
PAYLOAD_SLEEP="${PAYLOAD_SLEEP:-10}"

### The default config - run a small busybox image
# Define what we will be running (app under test)
#  Default is we run busybox, as a 'small' workload
PAYLOAD="${PAYLOAD:-busybox}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"
PAYLOAD_RUNTIME_ARGS="${PAYLOAD_RUNTIME_ARGS:- -m 2G}"


#########################
### Below are a couple of other examples of workload configs:
#  mysql is a more medium sized workload
#PAYLOAD="${PAYLOAD:-mysql}"
# Disable the aio use, or you can only run ~24 containers under runc, as you run out
# of handles in the kernel. Disable log-bin as it hits a file resize fail error.
#PAYLOAD_ARGS="${PAYLOAD_ARGS:- --innodb_use_native_aio=0 --disable-log-bin}"
#PAYLOAD_RUNTIME_ARGS="${PAYLOAD_RUNTIME_ARGS:- -m 4G -e MYSQL_ALLOW_EMPTY_PASSWORD=1}"
#
#  elasticsearch is a large workload
#PAYLOAD="${PAYLOAD:-elasticsearch}"
#PAYLOAD_ARGS="${PAYLOAD_ARGS:-}"
#PAYLOAD_RUNTIME_ARGS="${PAYLOAD_RUNTIME_ARGS:- -m 8G}"
#########################

###
# which RUNTIME we use is picked up from the env in
# common.bash. You can over-ride by setting RUNTIME in your env

###
# Define the cutoff checks for when we stop running the test
  # Run up to this many containers
MAX_NUM_CONTAINERS="${MAX_NUM_CONTAINERS:-20}"
  # Run until we have consumed this much memory (from MemFree)
MAX_MEMORY_CONSUMED="${MAX_MEMORY_CONSUMED:-6*1024*1024*1024}"
  # Run until we have this much MemFree left
MIN_MEMORY_FREE="${MIN_MEMORY_FREE:-2*1024*1024*1024}"

# These paths come from the lib common.bash init sequence
#PROXY_PATH
#SHIM_PATH
#HYPERVISOR_PATH

# We monitor dockerd as we know it can grow as we run containers
DOCKERD_PATH="${DOCKERD_PATH:-/usr/bin/dockerd}"

# Tools we need to have installed in order to operate
REQUIRED_COMMANDS="smem awk"

# If we 'dump' the system caches before we measure then we get less
# noise in the results - they show more what our un-reclaimable footprint is
DUMP_CACHES="${DUMP_CACHES:-1}"

# Affects the name of the file to store the results in
TEST_NAME="${TEST_NAME:-footprint-${PAYLOAD}}"

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
	check_cmds $REQUIRED_COMMANDS
	check_images "$PAYLOAD"

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
		"payload_runtime_args": "${PAYLOAD_RUNTIME_ARGS}",
		"payload_sleep": ${PAYLOAD_SLEEP},
		"max_containers": ${MAX_NUM_CONTAINERS},
		"max_memory_consumed": "${MAX_MEMORY_CONSUMED}",
		"min_memory_free": "${MIN_MEMORY_FREE}",
		"dockerd_path": "${DOCKERD_PATH}",
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

	docker kill $(docker ps -qa)
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

# Get the USS footprint of the VM runtime runtime components
function grab_vm_uss() {
	proxy=$(get_proc_uss $PROXY_PATH)
	shim=$(get_proc_uss $SHIM_PATH)
	qemu=$(get_proc_uss $HYPERVISOR_PATH)
	virtiofsd=$(get_proc_uss $VIRTIOFSD_PATH)

	total=$((proxy + shim + qemu + virtiofsd))

	local json="$(cat << EOF
		"uss": {
			"proxy": $proxy,
			"shim": $shim,
			"qemu": "$qemu",
			"virtiofsd": "$virtiofsd",
			"total": $total,
			"Units": "KB"
		}
EOF
)"

	metrics_json_add_array_fragment "$json"
}

# Get the PSS footprint of the VM runtime components
function grab_vm_pss() {
	proxy=$(get_proc_pss $PROXY_PATH)
	shim=$(get_proc_pss $SHIM_PATH)
	qemu=$(get_proc_pss $HYPERVISOR_PATH)
	virtiofsd=$(get_proc_pss $VIRTIOFSD_PATH)

	total=$((proxy + shim + qemu + virtiofsd))

	local json="$(cat << EOF
		"pss": {
			"proxy": $proxy,
			"shim": $shim,
			"qemu": "$qemu",
			"virtiofsd": "$virtiofsd",
			"total": $total,
			"Units": "KB"
		}
EOF
)"

	metrics_json_add_array_fragment "$json"
}

# Get the PSS footprint of dockerd - we know it can
# grow in size as we launch containers, so let's try to
# account for it
function grab_dockerd_pss() {
	item=$(get_proc_pss $DOCKERD_PATH)

	local json="$(cat << EOF
		"dockerd": {
			"pss": $item,
			"Units": "KB"
		}
EOF
)"

	metrics_json_add_array_fragment "$json"
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
		# USS taken by CC components
	grab_vm_uss
		# PSS taken all userspace
	grab_vm_pss
		# PSS taken all userspace
	grab_all_pss
		# PSS taken by dockerd
	grab_dockerd_pss
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

function go() {
	# Init the json cycle for this save
	metrics_json_start_array

	for i in $(seq 1 $MAX_NUM_CONTAINERS); do
		docker run --rm -tid --runtime=$RUNTIME $PAYLOAD_RUNTIME_ARGS $PAYLOAD $PAYLOAD_ARGS

		if [[ $PAYLOAD_SLEEP ]]; then
			sleep $PAYLOAD_SLEEP
		fi

		grab_stats

		# check if we have hit one of our limits and need to wrap up the tests
		if (($(check_limits))); then
			# Wrap up the results array
			metrics_json_end_array "Results"
			return
		fi
	done

	# Wrap up the results array
	metrics_json_end_array "Results"
}


function show_vars()
{
	echo -e "\nEvironment variables:"
	echo -e "\tName (default)"
	echo -e "\t\tDescription"
	echo -e "\tPAYLOAD (${PAYLOAD})"
	echo -e "\t\tThe docker image to run"
	echo -e "\tPAYLOAD_ARGS (${PAYLOAD_ARGS})"
	echo -e "\t\tAny arguments passed into the docker image"
	echo -e "\tPAYLOAD_RUNTIME_ARGS (${PAYLOAD_RUNTIME_ARGS})"
	echo -e "\t\tAny extra arguments passed into the docker 'run' command"
	echo -e "\tPAYLOAD_SLEEP (${PAYLOAD_SLEEP})"
	echo -e "\t\tSeconds to sleep between launch and measurement, to allow settling"
	echo -e "\tMAX_NUM_CONTAINERS (${MAX_NUM_CONTAINERS})"
	echo -e "\t\tThe maximum number of containers to run before terminating"
	echo -e "\tMAX_MEMORY_CONSUMED (${MAX_MEMORY_CONSUMED})"
	echo -e "\t\tThe maximum amount of memory to be consumed before terminating"
	echo -e "\tMIN_MEMORY_FREE (${MIN_MEMORY_FREE})"
	echo -e "\t\tThe minimum amount of memory allowed to be free before terminating"
	echo -e "\tDOCKERD_PATH (${DOCKERD_PATH})"
	echo -e "\t\tThe path to the Docker 'dockerd' binary (for 'smem' measurements)"
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

