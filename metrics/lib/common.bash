#!/bin/bash
#
# Copyright (c) 2017,2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

THIS_FILE=$(readlink -f ${BASH_SOURCE[0]})
LIB_DIR=${THIS_FILE%/*}
RESULT_DIR="${LIB_DIR}/../results"

source ${LIB_DIR}/json.bash

# Set variables to reasonable defaults if unset or empty
DOCKER_EXE="${DOCKER_EXE:-docker}"
RUNTIME="${RUNTIME:-kata-runtime}"

extract_kata_env(){
	local toml

	toml="$(kata-runtime kata-env)"

	# Actually the path to the runtime config - we don't know what runtime docker is
	# actually using...
	RUNTIME_PATH=$(awk '/^\[Runtime\]$/ {foundit=1} /^    Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
	RUNTIME_VERSION=$(awk '/^  \[Runtime.Version\]$/ {foundit=1} /^    Semver =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
	RUNTIME_COMMIT=$(awk '/^  \[Runtime.Version\]$/ {foundit=1} /^    Commit =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')

	SHIM_PATH=$(awk '/^\[Shim\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
	SHIM_VERSION=$(awk '/^\[Shim\]$/ {foundit=1} /^  Version =/ { if (foundit==1) {$1=$2=""; print $0; foundit=0} } ' <<< "$toml" | sed 's/"//g')

	PROXY_PATH=$(awk '/^\[Proxy\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
	PROXY_VERSION=$(awk '/^\[Proxy\]$/ {foundit=1} /^  Version =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')

	HYPERVISOR_PATH=$(awk '/^\[Hypervisor\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
	HYPERVISOR_VERSION=$(awk '/^\[Hypervisor\]$/ {foundit=1} /^  Version =/ { if (foundit==1) {$1=$2=""; print $0; foundit=0} } ' <<< "$toml" | sed 's/"//g')

}

# If we fail for any reason, exit through here and we should log that to the correct
# place and return the correct code to halt the run
die(){
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

# Sometimes we just want to warn about something - let's have a standard
# method for that, so maybe we can make a standard form that can be searched
# for in the logs/tooling
warning(){
	msg="$*"
	echo "WARNING: $msg" >&2
}

# This function checks existence of commands.
# They can be received standalone or as an array, e.g.
#
# cmds=(“cmd1” “cmd2”)
# check_cmds "${cmds[@]}"
check_cmds()
{
	local cmd req_cmds=( "$@" )
	for cmd in "${req_cmds[@]}"; do
		if ! command -v "$cmd" > /dev/null 2>&1; then
			die "command $cmd not available"
			exit 1;
		fi
		echo "command: $cmd: yes"
	done
}

# This function performs a docker pull on the image names
# passed in (notionally as 'about to be used'), to ensure
#  - that we have the most upto date images
#  - that any pull/refresh time (for a first pull) does not
#    happen during the test itself.
#
# The image list can be received standalone or as an array, e.g.
#
# images=(“img1” “img2”)
# check_imgs "${images[@]}"
check_images()
{
	local img req_images=( "$@" )
	for img in "${req_images[@]}"; do
		echo "docker pull'ing: $img"
		if ! docker pull "$img"; then
			die "Failed to docker pull image $img"
			exit 1;
		fi
		echo "docker pull'd: $img"
	done
}

# A one time (per uber test cycle) init that tries to get the
# system to a 'known state' as much as possible
metrics_onetime_init()
{
	# The onetime init must be called once, and only once
	if [ ! -z "$onetime_init_done" ]; then
		die "onetime_init() called more than once"
	fi

	# Restart services
	sudo systemctl restart docker

	# We want this to be seen in sub shells as well...
	# otherwise init_env() cannot check us
	export onetime_init_done=1
}

# Print a banner to the logs noting clearly which test
# we are about to run
test_banner()
{
	echo -e "\n===== starting test [$1] ====="
}

# Initialization/verification environment. This function makes
# minimal steps for metrics/tests execution.
init_env()
{
	test_banner "${TEST_NAME}"

	cmd=("docker")

	# check dependencies
	check_cmds "${cmd[@]}"

	# Remove all stopped containers
	clean_env

	# This clean up is more aggressive, this is in order to
	# decrease the factors that could affect the metrics results.
	kill_processes_before_start
}

# Clean environment, this function will try to remove all
# stopped/running containers, it is advisable to use this function
# in the final of each metrics test.
clean_env()
{
	containers_running=$(docker ps -q)

	if [ ! -z "$containers_running" ]; then
		# First stop all containers that are running
		sudo $DOCKER_EXE stop $containers_running

		# Remove all containers
		sudo $DOCKER_EXE rm -f $(docker ps -qa)
	fi
}

# This function checks if there are containers or
# shim/proxy/hypervisor processes up, if found, they are
# killed to start test with clean environment.
kill_processes_before_start() {
	DOCKER_PROCS=$(${DOCKER_EXE} ps -q)
	[[ -n "${DOCKER_PROCS}" ]] && clean_env

	result=$(check_processes "$HYPERVISOR_PATH")
	if [[ $result -ne 0 ]]; then
		warning "Found unexpected hypervisor [${HYPERVISOR_PATH}] processes present"
		# Sometimes we race and the process has gone by the time we list
		# it - so make a pgrep fail non-fatal
		pgrep -a -f "$HYPERVISOR_PATH" || true
		sudo killall -9 "${HYPERVISOR_PATH##*/}" || true
	fi

	result=$(check_processes "$SHIM_PATH")
	if [[ $result -ne 0 ]]; then
		warning "Found unexpected shim [${SHIM_PATH}] processes present"
		pgrep -a -f "$SHIM_PATH" || true
		sudo killall -9 "${SHIM_PATH##*/}" || true
	fi

	result=$(check_processes "$PROXY_PATH")
	if [[ $result -ne 0 ]]; then
		warning "Found unexpected proxy [${PROXY_PATH}] processes present"
		pgrep -a -f "$PROXY_PATH" || true
		sudo killall -9 "${PROXY_PATH##*/}" || true
	fi
}

# Check if process $1 is running or not
# Normally used to look for errant processes, and hence prints
# a warning
check_processes() {
	process=$1
	pgrep -f "$process"
	if [ $? -eq 0 ]; then
		warning "Found unexpected ${process} present"
		ps -ef | grep $process
		return 1
	fi
}

# Generate a random name - generally used when creating containers, but can
# be used for any other appropriate purpose
random_name() {
	mktemp -u kata-XXXXXX
}

common_init(){
	case "$RUNTIME" in
		kata-runtime)
			extract_kata_env
			;;
		*)
			warning "Unrecognised runtime ${RUNTIME}"
			;;
	esac
}

common_init
