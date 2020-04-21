#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../../lib/common.bash"
source "/etc/os-release" || "source /usr/lib/os-release"

# Environment variables
IMAGE="debian"
NETWORK_DRIVER="ipvlan"
SUBNET_ADDR="10.54.10.0/24"
FIRST_CONTAINER_NAME="containerA"
SECOND_CONTAINER_NAME="containerB"
FIRST_IP="10.54.10.2"
SECOND_IP="10.54.10.3"
PACKET_NUMBER="5"
PAYLOAD="tail -f /dev/null"

setup() {
	issue="https://github.com/kata-containers/runtime/issues/906"
	[ "${ID}" == "centos" ] || [ "${ID}" == "rhel" ] && skip "test not working with ${ID} see: ${issue}"

	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]

	docker_configuration_path="/etc/docker"
	# Check if directory exists
	if [ ! -d $docker_configuration_path ]; then
		sudo mkdir $docker_configuration_path
	fi

	# Check if daemon.json exists
	docker_configuration_file=$docker_configuration_path/daemon.json
	if [ -f $docker_configuration_file ]; then
		# Check experimental flag is enabled
		check_flag=$(grep '"experimental"\|true' $docker_configuration_file | wc -l)
		if  [ $check_flag -eq 0 ]; then
			# Enabling experimental flag at existing /etc/docker/daemon.json
			sed -i "2 i \  \"\experimental\"\: true," $docker_configuration_file
		fi
	else
		# Enabling experimental flag at /etc/docker/daemon.json
		echo '{"experimental":true}' | sudo tee $docker_configuration_file
	fi

	# Restart docker
	sudo systemctl restart docker
}

@test "ping container with ipvlan driver with mode l2" {
	issue="https://github.com/kata-containers/runtime/issues/906"
	[ "${ID}" == "centos" ] || [ "${ID}" == "rhel" ] && skip "test not working with ${ID} see: ${issue}"

	NETWORK_NAME="ipvlan2"
	NETWORK_MODE="l2"

	# Create network
	docker network create -d ${NETWORK_DRIVER} --subnet=${SUBNET_ADDR} \
		-o ipvlan_mode=${NETWORK_MODE} ${NETWORK_NAME}

	# Run the first container
	docker run -d --runtime=kata-runtime --network=${NETWORK_NAME} --ip=${FIRST_IP} \
		--name=${FIRST_CONTAINER_NAME} --runtime=runc ${IMAGE} ${PAYLOAD}

	# Run the second container
	docker run -d --runtime=kata-runtime --network=${NETWORK_NAME} --ip=${SECOND_IP} \
		--name=${SECOND_CONTAINER_NAME} --runtime=runc ${IMAGE} ${PAYLOAD}

	# Ping to the first container
	run docker exec ${SECOND_CONTAINER_NAME} sh -c "ping -c ${PACKET_NUMBER} ${FIRST_IP}"
	[ "$status" -eq 0 ]
}

@test "ping container with ipvlan driver with mode l3" {
	issue="https://github.com/kata-containers/runtime/issues/906"
	[ "${ID}" == "centos" ] || [ "${ID}" == "rhel" ] && skip "test not working with ${ID} see: ${issue}"

	NETWORK_NAME="ipvlan3"
	NETWORK_MODE="l3"

	# Create network
	docker network  create -d ${NETWORK_DRIVER} --subnet=${SUBNET_ADDR} \
		-o ipvlan_mode=${NETWORK_MODE} ${NETWORK_NAME}

	# Run the first container
	docker run -d --runtime=kata-runtime --network=${NETWORK_NAME} --ip=${FIRST_IP} \
		--name=${FIRST_CONTAINER_NAME} --runtime=runc ${IMAGE} ${PAYLOAD}

	# Run the second container
	docker run -d --runtime=kata-runtime --network=${NETWORK_NAME} --ip=${SECOND_IP} \
		--name=${SECOND_CONTAINER_NAME} --runtime=runc ${IMAGE} ${PAYLOAD}

	# Ping to the first container
	run docker exec ${SECOND_CONTAINER_NAME} sh -c "ping -c ${PACKET_NUMBER} ${FIRST_IP}"
	[ "$status" -eq 0 ]
}

teardown() {
	issue="https://github.com/kata-containers/runtime/issues/906"
	[ "${ID}" == "centos" ] && skip "test not working with ${ID} see: ${issue}"

	clean_env

	# Remove network
	docker network rm ${NETWORK_NAME}

	# Remove experimental flag
	check_which_flag=$(grep -c -x '{"experimental":true}' $docker_configuration_file)
	if [ $check_which_flag -eq 1 ]; then
		rm -rf $docker_configuration_path
	else
		sed -i 's|"experimental": true,||' $docker_configuration_file
	fi

	# Restart daemon to avoid issues in
	# docker that says `start too quickly`
	sudo systemctl daemon-reload

	sudo systemctl restart docker

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
