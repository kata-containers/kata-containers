#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# This file contains common functions that
# are being used by our metrics and integration tests

# Place where virtcontainers keeps its active pod info
VC_POD_DIR="${VC_POD_DIR:-/var/lib/vc/sbs}"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

die(){
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

info() {
        echo -e "INFO: $*"
}

# Gets versions and paths of all the components
# list in kata-env
extract_kata_env(){
	local toml

	# If we cannot find the runtime, or it fails to run for some reason, do not die
	# on the error, but set some sane defaults
	toml="$(set +e; kata-runtime kata-env)"
	if [ $? != 0 ]; then
		# We could be more diligent here and search for each individual component,
		# but if the runtime cannot tell us the exact details it is configured for then
		# we would be guessing anyway - so, set some defaults that may be true and give
		# strong hints that we 'made them up'.
		info "Runtime environment not found - setting defaults"
		RUNTIME_CONFIG_PATH="/usr/share/defaults/kata-containers/configuration.toml"
		RUNTIME_VERSION="0.0.0"
		RUNTIME_COMMIT="unknown"
		RUNTIME_PATH="/usr/local/bin/kata-runtime"
		SHIM_PATH="/usr/libexec/kata-containers/kata-shim"
		SHIM_VERSION="0.0.0"
		PROXY_PATH="/usr/libexec/kata-containers/kata-proxy"
		PROXY_VERSION="0.0.0"
		if [ "$KATA_HYPERVISOR" == firecracker ]; then
			HYPERVISOR_PATH="/usr/bin/firecracker"
		 else
			HYPERVISOR_PATH="/usr/bin/qemu-system-x86_64"
		fi
		HYPERVISOR_VERSION="0.0.0"
		INITRD_PATH=""
		NETMON_PATH="/usr/libexec/kata-containers/kata-netmon"
		return 0
	fi

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
}

# Checks that processes are not running
check_processes() {
	extract_kata_env
	vsock_configured=$($RUNTIME_PATH kata-env | awk '/UseVSock/ {print $3}')
	vsock_supported=$($RUNTIME_PATH kata-env | awk '/SupportVSock/ {print $3}')
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
