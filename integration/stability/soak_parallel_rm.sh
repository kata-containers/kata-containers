#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will run a number of parallel containers, and then try to
# 'rm -f' them all at the same time. It will check after each run and
# rm that we have the expected number of containers, shims, proxys,
# qemus and runtimes active
# The goals are two fold:
# - spot any stuck or non-started components
# - catch any hang ups

cidir=$(dirname "$0")
source "${cidir}/../../metrics/lib/common.bash"

# How many times will we run the test loop...
ITERATIONS="${ITERATIONS:-5}"

# the system 'free available' level where we stop running the tests, as otherwise
#  the system can crawl to a halt, and/or start refusing to launch new VMs anyway
# We choose 2G, as that is one of the default VM sizes for Kata
MEM_CUTOFF="${MEM_CUTOFF:-(2*1024*1024*1024)}"

# The default container/workload we run for testing
# nginx has show up issues in the past, for whatever reason, so
# let's default to that
PAYLOAD="${PAYLOAD:-nginx}"

# do we need a command argument for this payload?
COMMAND="${COMMAND:-}"

# Set the runtime if not set already
RUNTIME="${RUNTIME:-kata-runtime}"
RUNTIME_PATH=$(command -v $RUNTIME)

# And set the names of the processes we look for
QEMU_NAME="${QEMU_NAME:-qemu-lite-system-x86_64}"
RUNTIME_NAME="${RUNTIME_NAME:-kata-runtime}"
SHIM_NAME="${SHIM_NAME:-kata-shim}"
PROXY_NAME="${PROXY_NAME:-kata-proxy}"

# The place where virtcontainers keeps its active pod info
# This is ultimately what 'kata-runtime list' uses to get its info, but
# we can also check it for sanity directly
VC_POD_DIR="${VC_POD_DIR:-/var/lib/vc/sbs}"

# let's cap the test. If you want to run until you hit the memory limit
# then just set this to a very large number
MAX_CONTAINERS="${MAX_CONTAINERS:-110}"

count_containers() {
	docker ps -qa | wc -l
}

check_all_running() {
	local goterror=0

	echo "Checking ${how_many} containers have all relevant components"

	# check what docker thinks
	how_many_running=$(count_containers)

	if (( ${how_many_running} != ${how_many} )); then
		echo "Wrong number of containers running (${how_many_running} != ${how_many}) - stopping"
		((goterror++))
	fi

	# Only check for Kata components if we are using a Kata runtime
	if (( $check_kata_components )); then
		# Check we have one proxy per container
		how_many_proxys=$(ps --no-header -C ${PROXY_NAME} | wc -l)
		if (( ${how_many_running} != ${how_many_proxys} )); then
			echo "Wrong number of proxys running (${how_many_running} containers, ${how_many_proxys} proxys) - stopping"
			((goterror++))
		fi

		# check we have the right number of shims
		how_many_shims=$(ps --no-header -C ${SHIM_NAME} | wc -l)
		# one shim process per container...
		if (( ${how_many_running} != ${how_many_shims} )); then
			echo "Wrong number of shims running (${how_many_running} != ${how_many_shims}) - stopping"
			((goterror++))
		fi

		# check we have the right number of qemu's
		how_many_qemus=$(ps --no-header -C ${QEMU_NAME} | wc -l)
		if (( ${how_many_running} != ${how_many_qemus} )); then
			echo "Wrong number of qemus running (${how_many_running} != ${how_many_qemus}) - stopping"
			((goterror++))
		fi

		# check we have no runtimes running (they should be transient, we should not 'see them')
		how_many_runtimes=$(ps --no-header -C ${RUNTIME_NAME} | wc -l)
		if (( ${how_many_runtimes} )); then
			echo "Wrong number of runtimes running (${how_many_runtimes}) - stopping"
			((goterror++))
		fi

		# check how many containers the runtime list thinks we have
		num_list=$(sudo ${RUNTIME_PATH} list -q | wc -l)
		if (( ${how_many_running} != ${num_list} )); then
			echo "Wrong number of 'runtime list' containers running (${how_many_running} != ${num_list}) - stopping"
			((goterror++))
		fi

		# if this is kata-runtime, check how many pods virtcontainers thinks we have
		if [[ "$RUNTIME" == "kata-runtime" ]]; then
			num_vc_pods=$(sudo ls -1 ${VC_POD_DIR} | wc -l)

			if (( ${how_many_running} != ${num_vc_pods} )); then
				echo "Wrong number of pods in $VC_POD_DIR (${how_many_running} != ${num_vc_pods}) - stopping)"
				((goterror++))
			fi
		fi
	fi

	if (( goterror != 0 )); then
		show_system_state
		die "Got $goterror errors, quitting"
	fi
}

# reported system 'available' memory
get_system_avail() {
	echo $(free -b | head -2 | tail -1 | awk '{print $7}')
}

go() {
	echo "Running..."

	how_many=0

	while true; do {
		check_all_running

		echo "Run $RUNTIME: $PAYLOAD: $COMMAND"
		docker run --runtime=${RUNTIME} -tid ${PAYLOAD} ${COMMAND}

		((how_many++))
		if (( ${how_many} >= ${MAX_CONTAINERS} )); then
			echo "And we have hit the max ${how_many} containers"
			return
		fi

		how_much=$(get_system_avail)
		if (( ${how_much} < ${MEM_CUTOFF} )); then
			echo "And we are out of memory on container ${how_many} (${how_much} < ${MEM_CUTOFF})"
			return
		fi
	}
	done
}

kill_all_containers() {
	present=$(docker ps -qa | wc -l)
	if ((${present})); then
		docker rm -f $(docker ps -qa)
	fi
}

count_mounts() {
	echo $(mount | wc -l)
}

check_mounts() {
	final_mount_count=$(count_mounts)

	if [[ $final_mount_count != $initial_mount_count ]]; then
		echo "Final mount count does not match initial count (${final_mount_count} != ${initial_mount_count})"
	fi
}

init() {
	kill_all_containers

	# remember how many mount points we had before we do anything
	# and then sanity check we end up with no new ones dangling at the end
	initial_mount_count=$(count_mounts)

	# Only check Kata items if we are using a Kata runtime
	if [[ "$RUNTIME" == "kata-runtime" ]]; then
		echo "Checking Kata runtime $RUNTIME"
		check_kata_components=1
	else
		echo "Not a Kata runtime, not checking for Kata components"
		check_kata_components=0
	fi
}

spin() {
	for ((i=1; i<= ITERATIONS; i++)); do {
		echo "Start iteration $i of $ITERATIONS"
		#spin them up
		go
		#check we are in a sane state
		check_all_running
		#shut them all down
		kill_all_containers
		#Note there should be none running
		how_many=0
		#and check they all died
		check_all_running
		#and that we have no dangling mounts
		check_mounts
	}
	done
}

init
spin
