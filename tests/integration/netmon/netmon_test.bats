#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

# Environment variables
IMAGE="busybox"
NETWORK_NAME="test"
CONTAINER_NAME="containerA"
PAYLOAD_ARGS="tail -f /dev/null"
MACHINETYPE="${MACHINETYPE:-pc}"
netmon_issue="https://github.com/kata-containers/runtime/issues/1486"

setup() {
	if [ "$KATA_HYPERVISOR" == "nemu" ]; then
		skip " issue: https://github.com/kata-containers/runtime/issues/1003"
	fi

	[ "$MACHINETYPE" == "q35" ] && skip "test not working see: ${netmon_issue}"

	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]

	extract_kata_env

	# Enabling netmon at the configuration file
	sudo sed -i -e 's/^#\(enable_netmon =\).*$/\1 true/g' ${RUNTIME_CONFIG_PATH}
}

@test "test netmon" {
	if [ "$KATA_HYPERVISOR" == "nemu" ]; then
		skip " issue: https://github.com/kata-containers/runtime/issues/1003"
	fi

	[ "$MACHINETYPE" == "q35" ] && skip "test not working see: ${netmon_issue}"

	# Create network
	docker network create $NETWORK_NAME

	# Run container
	docker run --runtime=$RUNTIME -d --name $CONTAINER_NAME $IMAGE $PAYLOAD_ARGS

	# Check the number of interfaces before network connect
	before_interfaces=$(docker exec $CONTAINER_NAME ip addr show | awk '/inet.*brd/{print $NF}' | wc -l)

	docker network connect $NETWORK_NAME $CONTAINER_NAME

	# Check the number of interfaces after network connect
	after_interfaces=$(docker exec $CONTAINER_NAME ip addr show | awk '/inet.*brd/{print $NF}' | wc -l)

	# Compare interface numbers before and after network connect
	[ "$after_interfaces" -gt "$before_interfaces" ]

	# Check the ip address of the network
	network_address=$(docker network inspect test | grep IPv4Address | cut -d '/' -f1 | cut -d '"' -f4)

	# Check the ip address of the container of the new interface
	container_address=$(docker inspect --format "{{.NetworkSettings.Networks.${NETWORK_NAME}.IPAddress}}" $CONTAINER_NAME)

	# Check ip address of the network matches ip address of the container with the new interface
	[ "$network_address" == "$container_address" ]

	docker network disconnect $NETWORK_NAME $CONTAINER_NAME

	# Check that we got back to the original number of interfaces
	final_interfaces=$(docker exec $CONTAINER_NAME ip addr show | awk '/inet.*brd/{print $NF}' | wc -l)

	[ "$before_interfaces" -eq "$final_interfaces" ]
}

teardown() {
	if [ "$KATA_HYPERVISOR" == "nemu" ]; then
		skip " issue: https://github.com/kata-containers/runtime/issues/1003"
	fi

	[ "$MACHINETYPE" == "q35" ] && skip "test not working see: ${netmon_issue}"

	docker stop "$CONTAINER_NAME"

	docker rm "$CONTAINER_NAME"

	run docker network rm "$NETWORK_NAME"
	echo "$output"
	[ "$status" -eq 0 ] || return 1

	extract_kata_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ] || return 1

	# Disabling netmon at the configuration file
	sudo sed -i -e 's/^\(enable_netmon =\).*$/#\1 true/g' ${RUNTIME_CONFIG_PATH}
}
