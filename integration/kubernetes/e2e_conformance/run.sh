#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This script runs the Sonobuoy e2e Conformance tests.
# Run this script once your K8s cluster is running.
# WARNING: it is prefered to use containerd as the 
# runtime interface instead of cri-o as we have seen
# errors with cri-o that still need to be debugged.

set -o errexit
set -o nounset
set -o pipefail

export KUBECONFIG=$HOME/.kube/config
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../../lib/common.bash"

RUNTIME="${RUNTIME:-kata-runtime}"

# Check if Sonobuoy is still running every 5 minutes.
WAIT_TIME=300

# Add a global timeout of 2 hours to stop the execution
# in case Sonobuoy gets hanged.
GLOBAL_TIMEOUT=$((WAIT_TIME*24))

create_kata_webhook() {
	pushd "${SCRIPT_PATH}/../../../kata-webhook" >> /dev/null
	# Create certificates for the kata webhook
	./create-certs.sh

	# Apply kata-webhook deployment
	kubectl apply -f deploy/
	popd
}

run_sonobuoy() {
	sonobuoy_repo="github.com/heptio/sonobuoy"
	go get -u "$sonobuoy_repo"

	# Run Sonobuoy e2e tests
	info "Starting sonobuoy execution."
	info "When using kata as k8s runtime, the tests take around 2 hours to finish."
	sonobuoy run

	start_time=$(date +%s)
	estimated_end_time=$((start_time + GLOBAL_TIMEOUT))

	# Wait for the sonobuoy pod to be running.
	kubectl wait --for condition=Ready pod sonobuoy -n heptio-sonobuoy

	while sonobuoy status | grep -Eq "running|pending" && [ "$(date +%s)" -le "$estimated_end_time" ]; do
		info "sonobuoy still running, sleeping $WAIT_TIME seconds"
		sleep "$WAIT_TIME"
	done

	# Retrieve results
	e2e_result_dir="$(mktemp -d /tmp/kata_e2e_results.XXXXX)"
	sonobuoy retrieve "$e2e_result_dir" || \
		die "Couldn't retrieve sonobuoy results, please check status using: sonobuoy status"
	pushd "$e2e_result_dir" >> /dev/null

	# Uncompress results
	ls | grep tar.gz | xargs tar -xvf
	e2e_result_log="${e2e_result_dir}/plugins/e2e/results/e2e.log"
	info "Results of the e2e tests can be found on: $e2e_result_log"

	# If on CI, display the e2e log on the console.
	[ "$CI" == true ] && cat "$e2e_result_log"

	# Check for Success message on the logs.
	grep -aq " 0 Failed" "$e2e_result_log"
	grep -aq "SUCCESS" "$e2e_result_log" && \
		info " k8s e2e conformance using Kata runtime finished successfully"
	popd
}

cleanup() {
	# Remove sonobuoy execution pods
	sonobuoy delete
	info "Results directory $e2e_result_dir will not be deleted"
}

main() {
	if [ "$RUNTIME" == "kata-runtime" ]; then
		create_kata_webhook
	fi
	run_sonobuoy
	cleanup
}

main
