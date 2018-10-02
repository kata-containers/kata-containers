#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../../lib/common.bash"

# Image for macvlan testing
IMAGE="debian"
# Network name
NETWORK_NAME="macvlan1"
# Network driver
NETWORK_DRIVER="macvlan"
# Containers names
FIRST_CONTAINER_NAME="containerA"
SECOND_CONTAINER_NAME="containerB"
# Number of packets
PACKET_NUMBER="5"

setup () {
	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}

@test "ping container with macvlan driver" {
	# Create network
	docker network create -d ${NETWORK_DRIVER} ${NETWORK_NAME}

	# Run the first container
	docker run -d --runtime=${RUNTIME} --network=${NETWORK_NAME} --name=${FIRST_CONTAINER_NAME} ${IMAGE} sh -c "tail -f /dev/null"

	# Verify ip address
	ip_address=$(docker inspect --format "{{.NetworkSettings.IPAddress}}" ${FIRST_CONTAINER_NAME})
	if [ -z "$ip_address" ]; then
		echo >&2 "ERROR: Container ip address not found"
		exit 1
	fi

	# Ping to the first container
	run docker run --runtime=${RUNTIME} --network=${NETWORK_NAME} --name=${SECOND_CONTAINER_NAME} ${IMAGE} sh -c "ping -c ${PACKET_NUMBER} ${ip_address}"
	[ "$status" -eq 0 ]
}

teardown() {
	# Remove network
	docker network rm ${NETWORK_NAME}

	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
