#!/bin/bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../metrics/lib/common.bash"

set -x

replicas="${replicas:-8}"
deployment_name="${deployment_name:-deploymenttest}"
# How many times will we run the test loop...
iterations="${iterations:-10}"

function delete_deployment() {
	kubectl delete deployment "${deployment_name}"
}

function go() {
	kubectl scale deployment/"${deployment_name}" --replicas="${replicas}"
	cmd="kubectl get deployment/${deployment_name} -o yaml | grep 'availableReplicas: ${replicas}'"
	waitForProcess "300" "30" "${cmd}"
}

function init() {
	kubectl create -f "${SCRIPT_PATH}/runtimeclass_workloads/pod-deployment.yaml"
	kubectl wait --for=condition=Available --timeout=100s deployment/"${deployment_name}"
}

function main() {
	check_processes
	local i=0
	for (( i=1; i<="${iterations}"; i++ )); do
		info "Start iteration $i of $iterations"
		init
		#spin them up
		go
		#shut them all down
		delete_deployment
	done
}

main "$@"
