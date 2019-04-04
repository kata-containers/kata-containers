#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../../lib/common.bash"

IMAGE="busybox"
PAYLOAD="tail -f /dev/null"
NAME="test"
KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
issue="https://github.com/kata-containers/runtime/issues/1197"
net_issue="https://github.com/kata-containers/runtime/issues/906"

setup () {
	[ "${KATA_HYPERVISOR}" = "firecracker" ] && skip "test not working see: ${issue}"
	[ "${ID}" == "rhel" ] && skip "test not working with ${ID} see: ${net_issue}"
	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}

@test "Disable_new_netns equal to false" {
	[ "${KATA_HYPERVISOR}" = "firecracker" ] && skip "test not working see: ${issue}"
	[ "${ID}" == "rhel" ] && skip "test not working with ${ID} see: ${net_issue}"
	extract_kata_env

	sudo sed -i 's/#disable_new_netns = true/disable_new_netns = false/g' ${RUNTIME_CONFIG_PATH}

	# Run a container with no network
	docker run -d --runtime=${RUNTIME} --name=${NAME} --net=none ${IMAGE} ${PAYLOAD}

	# Check namespaces of host init daemon with no network
	network_ns=$(sudo stat -L -c "%i" /proc/1/ns/net)

	# Check namespaces of the processes (shim and hypervisor)
	general_processes=( ${SHIM_PATH} ${HYPERVISOR_PATH} )
	for i in "${general_processes[@]}"; do
		process_pid=$(pgrep -f "$i")
		process_ns=$(sudo stat -L -c "%i" /proc/$process_pid/ns/net)
		# Compare namespace of host init daemon is not equal to namespace of the process
		[ $process_ns -ne $network_ns ]
	done

	# Remove container
	docker rm -f $NAME

	sudo sed -i 's/disable_new_netns = false/#disable_new_netns = true/g' ${RUNTIME_CONFIG_PATH}
}

@test "Disable net" {
	[ "${KATA_HYPERVISOR}" = "firecracker" ] && skip "test not working see: ${issue}"
	[ "${ID}" == "rhel" ] && skip "test not working with ${ID} see: ${net_issue}"
	extract_kata_env

	# Get the name of the network name at the configuration.toml
	NETWORK_NAME=$(grep -E "internetworking_model=" ${RUNTIME_CONFIG_PATH} | head -1 | cut -d '"' -f2)

	# Disable the network
	sudo sed -i 's/#disable_new_netns = true/disable_new_netns = true/g' ${RUNTIME_CONFIG_PATH}
	sudo sed -i 's/internetworking_model=".*"/internetworking_model="none"/g' ${RUNTIME_CONFIG_PATH}

	# Run a container without network
	docker run -d --runtime=${RUNTIME} --name=${NAME} --net=none ${IMAGE} ${PAYLOAD}

	# Check namespaces of host init daemon with no network
	no_network_ns=$(sudo stat -L -c "%i" /proc/1/ns/net)

	# Check namespaces of the processes with no network
	general_processes=( ${PROXY_PATH} ${HYPERVISOR_PATH} ${SHIM_PATH} )
	for i in "${general_processes[@]}"; do
		process_pid=$(pgrep -f "$i")
		process_ns=$(sudo stat -L -c "%i" /proc/$process_pid/ns/net)
		# Compare namespace of host init daemon is equal to namespace of the process
		[ "$no_network_ns" == "$process_ns" ]
	done

	# Remove container
	docker rm -f $NAME

	# Restart the network at the configuration.toml
	sudo sed -i 's/disable_new_netns = true/#disable_new_netns = true/g' ${RUNTIME_CONFIG_PATH}
	sudo sed -i 's/internetworking_model="none"/internetworking_model="'"${NETWORK_NAME}"'"/g' ${RUNTIME_CONFIG_PATH}
}

teardown() {
	[ "${KATA_HYPERVISOR}" = "firecracker" ] && skip "test not working see: ${issue}"
	[ "${ID}" == "rhel" ] && skip "test not working with ${ID} see: ${net_issue}"
	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
