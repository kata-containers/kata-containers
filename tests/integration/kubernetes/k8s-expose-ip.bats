#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#
# Test that IP addresses/connections of PODS are routed/exposed correctly 
# via a loadbalancer service.
#
# This test is temporarily turned off on ARM CI.
# See detailed info in PR#2157(https://github.com/kata-containers/tests/pull/2157)

load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"

setup() {
	deployment="hello-world"
	service="my-service"
	agnhost_name=$(get_test_version "container_images.agnhost.name")
	agnhost_version=$(get_test_version "container_images.agnhost.version")

	get_pod_config_dir
}

@test "Expose IP Address" {

	# Create deployment
	sed -e "s#\${agnhost_image}#${agnhost_name}:${agnhost_version}#" \
		"${pod_config_dir}/deployment-expose-ip.yaml" |\
		kubectl create -f -

	# Check deployment creation
	cmd="kubectl wait --for=condition=Available --timeout=$timeout deployment/${deployment}"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Check pods are running
	cmd="kubectl get pods -o jsonpath='{.items[*].status.phase}' | grep Running"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Expose deployment
	kubectl expose deployment/${deployment} --type=LoadBalancer --name=${service}

	# There appears to be no easy way to formally wait for a loadbalancer service
	# to become 'ready' - there is no service.status.condition field to wait on.

	# Now obtain the local IP:port pair of the loadbalancer service and ensure
	# we can curl from it, and get the expected result
	svcip=$(kubectl get service ${service} -o=json | jq '.spec.clusterIP' | sed 's/"//g')
	svcport=$(kubectl get service ${service} -o=json | jq '.spec.ports[].port')
	# And check we can curl the expected response from that IP address
	echo_msg="hello,world"
	cmd="curl http://$svcip:$svcport/echo?msg=${echo_msg} | grep \"$echo_msg\""

	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# NOTE - we do not test the 'public IP' address of the node balancer here as
	# that may not be set up, as it may require an IP/DNS allocation and local
	# routing and firewall rules to access.
}

teardown() {
	kubectl delete services ${service}
	kubectl delete deployment ${deployment}
}
