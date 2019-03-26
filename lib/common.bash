#!/bin/bash
#
# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# This file contains common functions that
# are being used by our metrics and integration tests

# Place where virtcontainers keeps its active pod info
VC_POD_DIR="${VC_POD_DIR:-/var/lib/vc/sbs}"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

die() {
	local msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

warn() {
	local msg="$*"
	echo "WARNING: $msg"
}

info() {
	local msg="$*"
	echo "INFO: $msg"
}

# Check if the $1 argument is the name of a 'known'
# Kata runtime. Of course, the end user can choose any name they
# want in reality, but this function knows the names of the default
# and recommended Kata docker runtime install names.
is_a_kata_runtime(){
	case "$1" in
	"kata-runtime") ;&	# fallthrough
	"kata-qemu") ;&		# fallthrough
	"kata-fc")
		echo "1"
		return
		;;
	esac

	echo "0"
}


# Try to find the real runtime path for the docker runtime passed in $1
get_docker_kata_path(){
	local jpaths=$(docker info --format "{{json .Runtimes}}" || true)
	local rpath=$(jq .\"$1\".path <<< "$jpaths")
	# Now we have to de-quote it..
	rpath="${rpath%\"}"
	rpath="${rpath#\"}"
	echo "$rpath"
}

# Gets versions and paths of all the components
# list in kata-env
extract_kata_env(){
	local toml
	local rpath=$(get_docker_kata_path "$RUNTIME")
	if [ -n "$rpath" ]; then
		rpath=$(command -v "$rpath")
	fi

	# If we can execute the path handed back to us
	if [ -x "$rpath" ]; then
		# and if the kata-env command does not error out. Bash hack so we can get $? even
		# when the sub-command fails, but does not invoke the errexit in this parent shell.
		local is_valid=$( $rpath kata-env >/dev/null 2>&1 && echo $? || echo $? )

		if [ "$is_valid" == "0" ]; then
			# then we can parse out the data we want
			local toml="$($rpath kata-env)"

			# The runtime path itself, for kata-runtime, will be contained in the `kata-env`
			# section. For other runtimes we do not know where the runtime Docker is using lives.
			RUNTIME_CONFIG_PATH=$(awk '/^  \[Runtime.Config\]$/ {foundit=1} /^    Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
			RUNTIME_VERSION=$(awk '/^  \[Runtime.Version\]$/ {foundit=1} /^    Semver =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
			RUNTIME_COMMIT=$(awk '/^  \[Runtime.Version\]$/ {foundit=1} /^    Commit =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
			RUNTIME_PATH=$(awk '/^\[Runtime\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')

			SHIM_PATH=$(awk '/^\[Shim\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
			SHIM_VERSION=$(awk '/^\[Shim\]$/ {foundit=1} /^  Version =/ { if (foundit==1) {$1=$2=""; print $0; foundit=0} } ' <<< "$toml" | sed 's/"//g')

			PROXY_PATH=$(awk '/^\[Proxy\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
			PROXY_VERSION=$(awk '/^\[Proxy\]$/ {foundit=1} /^  Version =/ { if (foundit==1) {print $5; foundit=0} } ' <<< "$toml" | sed 's/"//g')

			HYPERVISOR_PATH=$(awk '/^\[Hypervisor\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
			HYPERVISOR_VERSION=$(awk '/^\[Hypervisor\]$/ {foundit=1} /^  Version =/ { if (foundit==1) {$1=$2=""; print $0; foundit=0} } ' <<< "$toml" | sed 's/"//g')

			INITRD_PATH=$(awk '/^\[Initrd\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')

			NETMON_PATH=$(awk '/^\[Netmon\]$/ {foundit=1} /^  Path =/ { if (foundit==1) {print $3; foundit=0} } ' <<< "$toml" | sed 's/"//g')
			return 0
		fi
	fi

	# We have not found a command with a 'kata-env' option we can run. Set up some
	# default values.
	# We could be more diligent here and search for each individual component,
	# but if the runtime cannot tell us the exact details it is configured for then
	# we would be guessing anyway - so, set some defaults that may be true and give
	# strong hints that we 'made them up'.
	info "Runtime environment not found - setting defaults"
	RUNTIME_CONFIG_PATH="/usr/share/defaults/kata-containers/configuration.toml"
	RUNTIME_VERSION="0.0.0"
	RUNTIME_COMMIT="unknown"
	# If docker is broken, disabled or not installed then we may not get a runtime
	# path from it...
	if [ -z "$RUNTIME_PATH" ]; then
		RUNTIME_PATH="/usr/bin/kata-runtime"
	else
		RUNTIME_PATH="$rpath"
	fi
	SHIM_PATH="/usr/libexec/kata-containers/kata-shim"
	SHIM_VERSION="0.0.0"
	PROXY_PATH="/usr/libexec/kata-containers/kata-proxy"
	PROXY_VERSION="0.0.0"
	if [ "$KATA_HYPERVISOR" == firecracker ]; then
		HYPERVISOR_PATH="/usr/bin/firecracker"
	else
		# We would use $(${cidir}/kata-arch.sh -d) here but we don't know
		# that the callee has set up ${cidir} for us.
		HYPERVISOR_PATH="/usr/bin/qemu-system-$(uname -m)"
	fi
	HYPERVISOR_VERSION="0.0.0"
	INITRD_PATH=""
	NETMON_PATH="/usr/libexec/kata-containers/kata-netmon"
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
	if [ "$vsock_configured" == true ] && [ "$vsock_supported" == true ]; then
		general_processes=( ${HYPERVISOR_PATH} ${SHIM_PATH} )
	else
		general_processes=( ${PROXY_PATH} ${HYPERVISOR_PATH} ${SHIM_PATH} )
	fi
	for i in "${general_processes[@]}"; do
		if pgrep -f "$i"; then
			die "Found unexpected ${i} present"
		fi
	done
}

# Checks that pods were not left
check_pods() {
	if [ -d ${VC_POD_DIR} ]; then
		# Verify that pods were not left
		pods_number=$(ls ${VC_POD_DIR} | wc -l)
		if [ ${pods_number} -ne 0 ]; then
			die "${pods_number} pods left and found at ${VC_POD_DIR}"
		fi
	else
		echo "Not ${VC_POD_DIR} directory found"
	fi
}

# Check that runtimes are not running, they should be transient
check_runtimes() {
	runtime_number=$(ps --no-header -C ${RUNTIME} | wc -l)
	if [ ${runtime_number} -ne 0 ]; then
		die "Unexpected runtime ${RUNTIME} running"
	fi
}

# Clean environment, this function will try to remove all
# stopped/running containers.
clean_env()
{
	# If the timeout has not been set, default it to 30s
	# Docker has a built in 10s default timeout, so make ours
	# longer than that.
	KATA_DOCKER_TIMEOUT=${KATA_DOCKER_TIMEOUT:-30}
	containers_running=$(timeout ${KATA_DOCKER_TIMEOUT} docker ps -q)

	if [ ! -z "$containers_running" ]; then
		# First stop all containers that are running
		# Use kill, as the containers are generally benign, and most
		# of the time our 'stop' request ends up doing a `kill` anyway
		sudo timeout ${KATA_DOCKER_TIMEOUT} docker kill $containers_running

		# Remove all containers
		sudo timeout ${KATA_DOCKER_TIMEOUT} docker rm -f $(docker ps -qa)
	fi
}

get_pod_config_dir() {
	if kubectl get runtimeclass 2> /dev/null | grep -q "kata"; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
		info "k8s configured to use runtimeclass"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
		info "k8s configured to use trusted and untrusted annotations"
	fi
}
