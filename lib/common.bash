#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# This file contains common functions that
# are being used by our metrics and integration tests

die(){
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

# Gets versions and paths of all the components
# list in kata-env
extract_kata_env(){
	local toml

	toml="$(kata-runtime kata-env)"

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
}

# Checks that processes are not running
check_processes() {
	extract_kata_env
	general_processes=( ${PROXY_PATH} ${HYPERVISOR_PATH} ${SHIM_PATH} )
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
	containers_running=$(docker ps -q)

	if [ ! -z "$containers_running" ]; then
		# First stop all containers that are running
		# Use kill, as the containers are generally benign, and most
		# of the time our 'stop' request ends up doing a `kill` anyway
		sudo docker kill $containers_running

		# Remove all containers
		sudo docker rm -f $(docker ps -qa)
	fi
}
