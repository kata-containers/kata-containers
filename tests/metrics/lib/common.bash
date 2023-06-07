#!/bin/bash
#
# Copyright (c) 2017-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

THIS_FILE=$(readlink -f ${BASH_SOURCE[0]})
LIB_DIR=${THIS_FILE%/*}
RESULT_DIR="${LIB_DIR}/../results"

source ${LIB_DIR}/../../lib/common.bash
source ${LIB_DIR}/json.bash
source /etc/os-release || source /usr/lib/os-release

# Set variables to reasonable defaults if unset or empty
CTR_EXE="${CTR_EXE:-ctr}"
DOCKER_EXE="${DOCKER_EXE:-docker}"
CTR_RUNTIME="${CTR_RUNTIME:-io.containerd.kata.v2}"
RUNTIME="${RUNTIME:-containerd-shim-kata-v2}"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
TEST_REPO="${TEST_REPO:-github.com/kata-containers/tests}"
JSON_HOST="${JSON_HOST:-}"

KSM_BASE="/sys/kernel/mm/ksm"
KSM_ENABLE_FILE="${KSM_BASE}/run"
KSM_PAGES_FILE="${KSM_BASE}/pages_to_scan"
KSM_SLEEP_FILE="${KSM_BASE}/sleep_millisecs"
KSM_PAGES_SHARED="${KSM_BASE}/pages_shared"

http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"

# The settings we use for an 'aggresive' KSM setup
# Scan 1000 pages every 50ms - 20,000 pages/s
KSM_AGGRESIVE_PAGES=1000
KSM_AGGRESIVE_SLEEP=50

declare -A registries
registries[ubuntu]=\
"docker.io/library
public.ecr.aws/lts
mirror.gcr.io/library
quay.io/libpod"

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

# This function performs a pull on the image names
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
		echo "ctr pull'ing: $img"
		if ! sudo "${CTR_EXE}" image pull "$img"; then
			die "Failed to pull image $img"
		fi
		echo "ctr pull'd: $img"
	done
}

generate_build_dockerfile() {
	local dockerfile="$1"
	local image="$2"
	local map_key="$3"
	local text_to_replace="$4"
	local regs=(${registries["${map_key}"]})
	for r in ${regs[@]}; do
		sed 's|'${text_to_replace}'|'${r}'|g' \
			"${dockerfile}.in" > "${dockerfile}"
		if sudo "${DOCKER_EXE}" build --build-arg http_proxy="${http_proxy}" --build-arg https_proxy="${https_proxy}" --label "$image" --tag "${image}" -f "$dockerfile" "$dockerfile_dir"; then
			return 0
		fi
	done
	return 1
}

# This function performs a build on the image names
# passed in, to ensure that we have the latest changes from
# the dockerfiles
build_dockerfile_image()
{
	local image="$1"
	local dockerfile_path="$2"
	local dockerfile_dir=${2%/*}

	if [ -f "$dockerfile_path" ]; then
		echo "docker building $image"
		if ! sudo "${DOCKER_EXE}" build --build-arg http_proxy="${http_proxy}" --build-arg https_proxy="${https_proxy}" --label "$image" --tag "${image}" -f "$dockerfile_path" "$dockerfile_dir"; then
			die "Failed to docker build image $image"
		fi
		return 0
	fi

	generate_build_dockerfile "${dockerfile_path}" "${image}" "ubuntu" "@UBUNTU_REGISTRY@" \
		|| die "Failed to docker build image $image"
}

# This function removes the ctr image, builds a new one using a dockerfile
# and imports the image from docker to ctr
check_ctr_images()
{
	local ctr_image="$1"
	local dockerfile_path="$2"
	local docker_image="$(echo ${ctr_image} | cut -d/ -f3 | cut -d: -f1)"

	if [ -z "$ctr_image" ] || [ -z "$dockerfile_path" ]; then
		die "Missing image or dockerfile path variable"
	fi

	sudo "${CTR_EXE}" i rm "${ctr_image}"
	build_dockerfile_image "${docker_image}" "${dockerfile_path}"
	sudo "${DOCKER_EXE}" save -o "${docker_image}.tar" "${docker_image}"
	sudo "${CTR_EXE}" i import "${docker_image}.tar"
	rm -rf "${docker_image}".tar
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
	restart_docker_service
	restart_containerd_service

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

	cmd=("docker" "ctr")

	# check dependencies
	check_cmds "${cmd[@]}"

	# Remove all stopped containers
	clean_env
	clean_env_ctr

	# This clean up is more aggressive, this is in order to
	# decrease the factors that could affect the metrics results.
	kill_processes_before_start
}

# This function checks if there are containers or
# shim/proxy/hypervisor processes up, if found, they are
# killed to start test with clean environment.
kill_processes_before_start() {
	DOCKER_PROCS=$(sudo "${DOCKER_EXE}" ps -q)
	[[ -n "${DOCKER_PROCS}" ]] && clean_env

	CTR_PROCS=$(sudo "${CTR_EXE}" t list -q)
	[[ -n "${CTR_PROCS}" ]] && clean_env_ctr

	check_processes
}

# Generate a random name - generally used when creating containers, but can
# be used for any other appropriate purpose
random_name() {
	mktemp -u kata-XXXXXX
}

show_system_ctr_state() {
	echo "Showing system state:"
	echo " --Check containers--"
	sudo "${CTR_EXE}" c list
	echo " --Check tasks--"
	sudo "${CTR_EXE}" task list

	local processes="containerd-shim-kata-v2"

	for p in ${processes}; do
		echo " --pgrep ${p}--"
		pgrep -a ${p}
	done
}

common_init(){
	if [ "$CTR_RUNTIME" == "io.containerd.kata.v2" ] || [ "$RUNTIME" == "containerd-shim-kata-v2" ]; then
		extract_kata_env
	else
		# We know we have nothing to do for runc or shimv2
		if [ "$CTR_RUNTIME" != "io.containerd.runc.v2" ] || [ "$RUNTIME" != "runc" ]; then
			warn "Unrecognised runtime"
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

	if [ "${KATA_HYPERVISOR}" == "qemu" ]; then
		# Disable virtio-fs and save whether it was enabled previously
		set_virtio_out=$(sudo -E PATH="$PATH" "${LIB_DIR}/../../.ci/set_kata_config.sh" shared_fs virtio-9p)
		echo "${set_virtio_out}"
		grep -q "already" <<< "${set_virtio_out}" || was_virtio_fs=true;
	fi
}

restore_virtio_fs(){
	# Re-enable virtio-fs if it was enabled previously
	[ -n "${was_virtio_fs}" ] && sudo -E PATH="$PATH" "${LIB_DIR}/../../.ci/set_kata_config.sh" shared_fs virtio-fs || \
		info "Not restoring virtio-fs since it wasn't enabled previously"
}

restore_ksm_settings(){
	echo "restoring KSM settings"
	# First turn off the run to ensure if we are then re-enabling
	# that any changes take effect
	sudo bash -c "echo 0 > ${KSM_ENABLE_FILE}"
	sudo bash -c "echo ${ksm_stored_pages} > ${KSM_PAGES_FILE}"
	sudo bash -c "echo ${ksm_stored_sleep} > ${KSM_SLEEP_FILE}"
	sudo bash -c "echo ${ksm_stored_run} > ${KSM_ENABLE_FILE}"
	[ "${KATA_HYPERVISOR}" == "qemu" ] && restore_virtio_fs
}

disable_ksm(){
	echo "disabling KSM"
	sudo bash -c "echo 0 > ${KSM_ENABLE_FILE}"
	[ "${KATA_HYPERVISOR}" == "qemu" ] && restore_virtio_fs
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
	[[ "$RUNTIME" == "runc" ]] || [[ "$CTR_RUNTIME" == "io.containerd.runc.v2" ]] && return
	local t pcnt
	local oldscan=-1 newscan
	local oldpages=-1 newpages

	oldscan=$(cat /sys/kernel/mm/ksm/full_scans)

	# Wait some time for KSM to kick in to avoid early dismissal
	for ((t=0; t<5; t++)); do
		pages=$(cat "${KSM_PAGES_SHARED}")
		[[ "$pages" -ne 0 ]] && echo "Discovered KSM activity" && break
		sleep 1
	done

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

function start_kubernetes() {
	info "Start k8s"
	pushd "${GOPATH}/src/${TEST_REPO}/integration/kubernetes"
	bash ./init.sh
	popd
}

function end_kubernetes() {
	info "End k8s"
	pushd "${GOPATH}/src/${TEST_REPO}/integration/kubernetes"
	bash ./cleanup_env.sh
	popd
}

common_init
