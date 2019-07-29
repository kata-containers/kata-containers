#!/bin/bash
#
# Copyright (c) 2017,2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

THIS_FILE=$(readlink -f ${BASH_SOURCE[0]})
LIB_DIR=${THIS_FILE%/*}
RESULT_DIR="${LIB_DIR}/../results"

source ${LIB_DIR}/../../lib/common.bash
source ${LIB_DIR}/json.bash
source /etc/os-release || source /usr/lib/os-release
KATA_KSM_THROTTLER="${KATA_KSM_THROTTLER:-no}"

# Set variables to reasonable defaults if unset or empty
DOCKER_EXE="${DOCKER_EXE:-docker}"
RUNTIME="${RUNTIME:-kata-runtime}"

KSM_BASE="/sys/kernel/mm/ksm"
KSM_ENABLE_FILE="${KSM_BASE}/run"
KSM_PAGES_FILE="${KSM_BASE}/pages_to_scan"
KSM_SLEEP_FILE="${KSM_BASE}/sleep_millisecs"
KSM_PAGES_SHARED="${KSM_BASE}/pages_shared"

# The settings we use for an 'aggresive' KSM setup
# Scan 1000 pages every 50ms - 20,000 pages/s
KSM_AGGRESIVE_PAGES=1000
KSM_AGGRESIVE_SLEEP=50

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
		fi
		echo "docker pull'd: $img"
	done
}

# This function performs a docker build on the image names
# passed in, to ensure that we have the latest changes from
# the dockerfiles
build_dockerfile_image()
{
	local image="$1"
	local dockerfile_path="$2"
	local dockerfile_dir=${2%/*}

	echo "docker building $image"
	if ! docker build --label "$image" --tag "${image}" -f "$dockerfile_path" "$dockerfile_dir"; then
		die "Failed to docker build image $image"
	fi
}

# This function verifies that the dockerfile version is
# equal to the test version in order to build the image or
# just run the test
check_dockerfiles_images()
{
	local image="$1"
	local dockerfile_path="$2"

	if [ -z "$image" ] || [ -z "$dockerfile_path" ]; then
		die "Missing image or dockerfile path variable"
	fi

	# Verify that dockerfile version is equal to test version
	check_image=$(docker images "$image" -q)
	if [ -n "$check_image" ]; then
		# Check image label
		check_image_version=$(docker image inspect $image | grep -w DOCKERFILE_VERSION | head -1 | cut -d '"' -f4)
		if [ -n "$check_image_version" ]; then
			echo "$image is not updated"
			build_dockerfile_image "$image" "$dockerfile_path"
		else
			# Check dockerfile label
			dockerfile_version=$(grep DOCKERFILE_VERSION $dockerfile_path | cut -d '"' -f2)
			if [ "$dockerfile_version" != "$check_image_version" ]; then
				echo "$dockerfile_version is not equal to $check_image_version"
				build_dockerfile_image "$image" "$dockerfile_path"
			fi
		fi
	else
		build_dockerfile_image "$image" "$dockerfile_path"
	fi
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

# This function checks if there are containers or
# shim/proxy/hypervisor processes up, if found, they are
# killed to start test with clean environment.
kill_processes_before_start() {
	DOCKER_PROCS=$(${DOCKER_EXE} ps -q)
	[[ -n "${DOCKER_PROCS}" ]] && clean_env
	check_processes
}

# Generate a random name - generally used when creating containers, but can
# be used for any other appropriate purpose
random_name() {
	mktemp -u kata-XXXXXX
}

# Dump diagnostics about our current system state.
# Very useful for diagnosing if we have failed a sanity check
show_system_state() {
	echo "Showing system state:"
	echo " --Docker ps--"
	${DOCKER_EXE} ps -a
	echo " --${RUNTIME} list--"
	local RPATH=$(command -v ${RUNTIME})
	sudo ${RPATH} list

	local processes="kata-proxy kata-shim kata-runtime $(basename ${HYPERVISOR_PATH} | cut -d '-' -f1)"

	for p in ${processes}; do
		echo " --pgrep ${p}--"
		pgrep -a ${p}
	done

	# Verify kata-ksm-throttler
	if [ "$KATA_KSM_THROTTLER" == "yes" ]; then
		process="kata-ksm-throttler"
		process_path=$(whereis ${process} | tr -d '[:space:]'| cut -d ':' -f2)
		echo " --pgrep ${process}--"
		pgrep -f ${process_path} > /dev/null
	fi
}

common_init(){

	# If we are running a kata runtime, go extract its environment
	# for later use.
	local iskata=$(is_a_kata_runtime "$RUNTIME")

	if [ "$iskata" == "1" ]; then
		extract_kata_env
	else
		# We know we have nothing to do for runc
		if [ "$RUNTIME" != "runc" ]; then
			warning "Unrecognised runtime ${RUNTIME}"
		fi
	fi
}


# Save the current KSM settings so we can restore them later
save_ksm_settings(){
	echo "saving KSM settings"
	ksm_stored_run=$(cat ${KSM_ENABLE_FILE})
	ksm_stored_pages=$(cat ${KSM_ENABLE_FILE})
	ksm_stored_sleep=$(cat ${KSM_ENABLE_FILE})
}

set_ksm_aggressive(){
	echo "setting KSM to aggressive mode"
	# Flip the run off/on to ensure a restart/rescan
	sudo bash -c "echo 0 > ${KSM_ENABLE_FILE}"
	sudo bash -c "echo ${KSM_AGGRESIVE_PAGES} > ${KSM_PAGES_FILE}"
	sudo bash -c "echo ${KSM_AGGRESIVE_SLEEP} > ${KSM_SLEEP_FILE}"
	sudo bash -c "echo 1 > ${KSM_ENABLE_FILE}"
}

restore_ksm_settings(){
	echo "restoring KSM settings"
	# First turn off the run to ensure if we are then re-enabling
	# that any changes take effect
	sudo bash -c "echo 0 > ${KSM_ENABLE_FILE}"
	sudo bash -c "echo ${ksm_stored_pages} > ${KSM_PAGES_FILE}"
	sudo bash -c "echo ${ksm_stored_sleep} > ${KSM_SLEEP_FILE}"
	sudo bash -c "echo ${ksm_stored_run} > ${KSM_ENABLE_FILE}"
}

disable_ksm(){
	echo "disabling KSM"
	sudo bash -c "echo 0 > ${KSM_ENABLE_FILE}"
}

# See if KSM is enabled.
# If so, amend the test name to reflect that
check_for_ksm(){
	if [ ! -f ${KSM_ENABLE_FILE} ]; then
		return
	fi

	ksm_on=$(< ${KSM_ENABLE_FILE})

	if [ $ksm_on == "1" ]; then
		TEST_NAME="${TEST_NAME} ksm"
	fi
}

# Wait for KSM to settle down, or timeout waiting
# The basic algorithm is to look at the pages_shared value
# at the end of every 'full scan', and if the value
# has changed very little, then we are done (because we presume
# a full scan has managed to do few new merges)
#
# arg1 - timeout in seconds
wait_ksm_settle(){
	[[ "$RUNTIME" == "runc" ]] || [[ "$RUNTIME" == "kata-fc" ]] && return
	local t pcnt
	local oldscan=-1 newscan
	local oldpages=-1 newpages

	oldscan=$(cat /sys/kernel/mm/ksm/full_scans)

	# Go around the loop until either we see a small % change
	# between two full_scans, or we timeout
	for ((t=0; t<$1; t++)); do

		newscan=$(cat /sys/kernel/mm/ksm/full_scans)
		newpages=$(cat "${KSM_PAGES_SHARED}")
		[[ "$newpages" -eq 0 ]] && echo "No need to wait for KSM to settle" && return

		if (( newscan != oldscan )); then
			echo -e "\nnew full_scan ($oldscan to $newscan)"

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

common_init
