#!/usr/bin/env bats
# *-*- Mode: sh; sh-basic-offset: 8; indent-tabs-mode: nil -*-*
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Swarm testing : This will start swarm as well as it will create and
# run swarm replicas using Nginx

load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

# Image for swarm testing
nginx_image="gabyct/nginx"
# Name of service to test swarm
SERVICE_NAME="testswarm"
# Number of replicas that will be launch
number_of_replicas=1
# Timeout in seconds to verify replicas are running
timeout=10
# Retry number for the curl
number_of_retries=5

setup() {
	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]

	interfaces=$(basename -a /sys/class/net/*)
	swarm_interface_arg=""
	for i in ${interfaces[@]}; do
		check_ip_address=$(ip a show dev ${i} | awk '/inet / {print $2}' | cut -d'/' -f1)
		if [ "$(cat /sys/class/net/${i}/operstate)" == "up" ] && [ -n "${check_ip_address}" ]; then
			swarm_interface_arg="${check_ip_address}"
			break;
		fi
	done
	docker swarm init --advertise-addr "${swarm_interface_arg}"
	nginx_command="hostname > /usr/share/nginx/html/hostname; nginx -g \"daemon off;\""
	docker service create \
		--name "${SERVICE_NAME}" --replicas $number_of_replicas \
		--publish 8080:80 "${nginx_image}" sh -c "$nginx_command"
	running_regex='Running\s+\d+\s(seconds|minutes)\s+ago'
	for i in $(seq "$timeout") ; do
		docker service ls --filter name="$SERVICE_NAME"
		replicas_running=$(docker service ps "$SERVICE_NAME" | grep -c -P "${running_regex}")
		if [ "$replicas_running" -ge "$number_of_replicas" ]; then
			break
		fi
		sleep 1
		[ "$i" == "$timeout" ] && return 1
	done
}

@test "check_replicas_interfaces" {
	# here we are checking that each replica has two interfaces
	# and they should be always eth0 and eth1
	REPLICA_ID=$(docker ps -q)
	docker exec ${REPLICA_ID} sh -c "ip route show | grep -E eth0 && ip route show | grep -E eth1" > /dev/null
}

teardown() {
	docker service remove "${SERVICE_NAME}"
	docker swarm leave --force

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
