#!/usr/bin/env bash
#
# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# This file contains common functions that
# are being used by our metrics and integration tests

# Kata tests directory used for storing various test-related artifacts.
KATA_TESTS_BASEDIR="${KATA_TESTS_BASEDIR:-/var/log/kata-tests}"

# Directory that can be used for storing test logs.
KATA_TESTS_LOGDIR="${KATA_TESTS_LOGDIR:-${KATA_TESTS_BASEDIR}/logs}"

# Directory that can be used for storing test data.
KATA_TESTS_DATADIR="${KATA_TESTS_DATADIR:-${KATA_TESTS_BASEDIR}/data}"

# Directory that can be used for storing cache kata components
KATA_TESTS_CACHEDIR="${KATA_TESTS_CACHEDIR:-${KATA_TESTS_BASEDIR}/cache}"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

RUNTIME="${RUNTIME:-containerd-shim-kata-v2}"

die() {
	local msg="$*"
	echo -e "[$(basename $0):${BASH_LINENO[0]}] ERROR: $msg" >&2
	exit 1
}

warn() {
	local msg="$*"
	echo -e "[$(basename $0):${BASH_LINENO[0]}] WARNING: $msg"
}

info() {
	local msg="$*"
	echo -e "[$(basename $0):${BASH_LINENO[0]}] INFO: $msg"
}

handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo -e "[$(basename $0):$line_number] ERROR: $(eval echo "$BASH_COMMAND")"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

waitForProcess() {
	wait_time="$1"
	sleep_time="$2"
	cmd="$3"
	while [ "$wait_time" -gt 0 ]; do
		if eval "$cmd"; then
			return 0
		else
			sleep "$sleep_time"
			wait_time=$((wait_time-sleep_time))
		fi
	done
	return 1
}

# Check if the $1 argument is the name of a 'known'
# Kata runtime. Of course, the end user can choose any name they
# want in reality, but this function knows the names of the default
# and recommended Kata docker runtime install names.
is_a_kata_runtime() {
	if [ "$1" = "containerd-shim-kata-v2" ] || [ "$1" = "io.containerd.kata.v2" ]; then
		echo "1"
	else
		echo "0"
	fi
}

# Gets versions and paths of all the components
# list in kata-env
extract_kata_env() {
	RUNTIME_CONFIG_PATH=$(kata-runtime kata-env --json | jq -r .Runtime.Config.Path)
	RUNTIME_VERSION=$(kata-runtime kata-env --json | jq -r .Runtime.Version | grep Semver | cut -d'"' -f4)
	RUNTIME_COMMIT=$(kata-runtime kata-env --json | jq -r .Runtime.Version | grep Commit | cut -d'"' -f4)
	RUNTIME_PATH=$(kata-runtime kata-env --json | jq -r .Runtime.Path)

	# Shimv2 path is being affected by https://github.com/kata-containers/kata-containers/issues/1151
	SHIM_PATH=$(readlink $(command -v containerd-shim-kata-v2))
	SHIM_VERSION=${RUNTIME_VERSION}

	HYPERVISOR_PATH=$(kata-runtime kata-env --json | jq -r .Hypervisor.Path)
	# TODO: there is no kata-runtime of rust version currently
	if [ "${KATA_HYPERVISOR}" != "dragonball" ]; then
		HYPERVISOR_VERSION=$(sudo -E ${HYPERVISOR_PATH} --version | head -n1)
	fi
	VIRTIOFSD_PATH=$(kata-runtime kata-env --json | jq -r .Hypervisor.VirtioFSDaemon)

	INITRD_PATH=$(kata-runtime kata-env --json | jq -r .Initrd.Path)
}

# Checks that processes are not running
check_processes() {
	extract_kata_env

	# Only check the kata-env if we have managed to find the kata executable...
	if [ -x "$RUNTIME_PATH" ]; then
		local vsock_configured=$($RUNTIME_PATH kata-env | awk '/UseVSock/ {print $3}')
		local vsock_supported=$($RUNTIME_PATH kata-env | awk '/SupportVSock/ {print $3}')
	else
		local vsock_configured="false"
		local vsock_supported="false"
	fi

	general_processes=( ${HYPERVISOR_PATH} ${SHIM_PATH} )

	for i in "${general_processes[@]}"; do
		if pgrep -f "$i"; then
			die "Found unexpected ${i} present"
		fi
	done
}

# Clean environment, this function will try to remove all
# stopped/running containers.
clean_env()
{
	# If the timeout has not been set, default it to 30s
	# Docker has a built in 10s default timeout, so make ours
	# longer than that.
	KATA_DOCKER_TIMEOUT=${KATA_DOCKER_TIMEOUT:-30}
	containers_running=$(sudo timeout ${KATA_DOCKER_TIMEOUT} docker ps -q)

	if [ ! -z "$containers_running" ]; then
		# First stop all containers that are running
		# Use kill, as the containers are generally benign, and most
		# of the time our 'stop' request ends up doing a `kill` anyway
		sudo timeout ${KATA_DOCKER_TIMEOUT} docker kill $containers_running

		# Remove all containers
		sudo timeout ${KATA_DOCKER_TIMEOUT} docker rm -f $(docker ps -qa)
	fi
}

clean_env_ctr()
{
	local count_running="$(sudo ctr c list -q | wc -l)"
	local remaining_attempts=10
	declare -a running_tasks=()
	local count_tasks=0
	local sleep_time=1
	local time_out=10

	[ "$count_running" -eq "0" ] && return 0

	readarray -t running_tasks < <(sudo ctr t list -q)

	info "Wait until the containers gets removed"

	for task_id in "${running_tasks[@]}"; do
		sudo ctr t kill -a -s SIGTERM ${task_id} >/dev/null 2>&1
		sleep 0.5
	done

	# do not stop if the command fails, it will be evaluated by waitForProcess
	local cmd="[[ $(sudo ctr tasks list | grep -c "STOPPED") == "$count_running" ]]" || true

	local res="ok"
	waitForProcess "${time_out}" "${sleep_time}" "$cmd" || res="fail"

	[ "$res" == "ok" ] || sudo systemctl restart containerd

	while (( remaining_attempts > 0 )); do
		[ "${RUNTIME}" == "runc" ] && sudo ctr tasks rm -f $(sudo ctr task list -q)
		sudo ctr c rm $(sudo ctr c list -q) >/dev/null 2>&1

		count_running="$(sudo ctr c list -q | wc -l)"

		[ "$count_running" -eq 0 ] && break

		remaining_attempts=$((remaining_attempts-1))
	done

	count_tasks="$(sudo ctr t list -q | wc -l)"

	if (( count_tasks > 0 )); then
		die "Can't remove running contaienrs."
	fi
}

# Restarts a systemd service while ensuring the start-limit-burst is set to 0.
# Outputs warnings to stdio if something has gone wrong.
#
# Returns 0 on success, 1 otherwise
restart_systemd_service_with_no_burst_limit() {
	local service=$1
	info "restart $service service"

	local active=$(systemctl show "$service.service" -p ActiveState | cut -d'=' -f2)
	[ "$active" == "active" ] || warn "Service $service is not active"

	local start_burst=$(systemctl show "$service".service -p StartLimitBurst | cut -d'=' -f2)
	if [ "$start_burst" -ne 0 ]
	then
		local unit_file=$(systemctl show "$service.service" -p FragmentPath | cut -d'=' -f2)
		[ -f "$unit_file" ] || { warn "Can't find $service's unit file: $unit_file"; return 1; }

		local start_burst_set=$(sudo grep StartLimitBurst $unit_file | wc -l)
		if [ "$start_burst_set" -eq 0 ]
		then
			sudo sed -i '/\[Service\]/a StartLimitBurst=0' "$unit_file"
		else
			sudo sed -i 's/StartLimitBurst.*$/StartLimitBurst=0/g' "$unit_file"
		fi

		sudo systemctl daemon-reload
	fi

	sudo systemctl restart "$service"

	local state=$(systemctl show "$service.service" -p SubState | cut -d'=' -f2)
	[ "$state" == "running" ] || { warn "Can't restart the $service service"; return 1; }

	start_burst=$(systemctl show "$service.service" -p StartLimitBurst | cut -d'=' -f2)
	[ "$start_burst" -eq 0 ] || { warn "Can't set start burst limit for $service service"; return 1; }

	return 0
}

restart_containerd_service() {
	restart_systemd_service_with_no_burst_limit containerd || return 1

	local retries=5
	local counter=0
	until [ "$counter" -ge "$retries" ] || sudo ctr --connect-timeout 1s version > /dev/null 2>&1
	do
		info "Waiting for containerd socket..."
		((counter++))
	done

	[ "$counter" -ge "$retries" ] && { warn "Can't connect to containerd socket"; return 1; }

	clean_env_ctr
	return 0
}
