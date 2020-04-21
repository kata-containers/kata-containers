#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/openshiftrc"
load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	# Verify there is a node ready to run containers,
	# if not, sleep 5s and check again until maximum of 30s
	cmd="sudo -E oc get nodes | grep \" Ready\""
	wait_time=60
	sleep_time=5
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	pod_name="hello-openshift"
	image="openshift/${pod_name}"
	sudo -E crictl pull "$image"
	kata_runtime_bin=$(command -v kata-runtime)
}

@test "Hello Openshift using Kata Containers" {
	# The below json file was taken from:
	# https://github.com/openshift/origin/tree/master/examples/hello-openshift
	# and modified to be an untrusted workload and then use Kata Containers.
	sudo -E oc create -f "${BATS_TEST_DIRNAME}/data/hello-pod-kata.json"
	output_file=$(mktemp)
	cmd="sudo -E oc describe pod/${pod_name} | grep State | grep Running"
	# Wait for nginx service to come up
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	container_id=$(sudo -E oc describe pod/${pod_name} | grep "Container ID" | cut -d '/' -f3)
	# Verify that the running container is a Clear Container
	sudo -E "$kata_runtime_bin" list | grep "$container_id" | grep "running"
	# Verify connectivity
	container_ip=$(sudo -E oc get pod "${pod_name}" -o yaml | grep "podIP" | awk '{print $2}')
	container_port=$(sudo -E oc get pod "${pod_name}" -o yaml | grep "Port" | awk '{print $3}')
	curl "${container_ip}:${container_port}" &> "$output_file"
	grep "Hello OpenShift" "$output_file"
}

teardown() {
	sudo -E oc describe pod/${pod_name} | grep State
	rm "$output_file"
	sudo -E oc delete pod "$pod_name"
	# Wait for the pod to be deleted
	cmd="sudo -E oc get pods | grep found."
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	sudo -E oc get pods
}
