#!/bin/bash
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

kubernetes_dir=$(dirname "$(readlink -f "$0")")
source "${kubernetes_dir}/../../common.bash"

cleanup() {
	true
}

trap cleanup EXIT

# Setting to "yes" enables fail fast, stopping execution at the first failed test.
K8S_TEST_FAIL_FAST="${K8S_TEST_FAIL_FAST:-no}"
K8S_TEST_NV=("k8s-nvidia-nim.bats")

ensure_yq

info "Running tests with bats version: $(bats --version)"

tests_fail=()
for K8S_TEST_ENTRY in "${K8S_TEST_NV[@]}"
do
	K8S_TEST_ENTRY=$(echo "${K8S_TEST_ENTRY}" | tr -d '[:space:][:cntrl:]')
	info "$(kubectl get pods --all-namespaces 2>&1)"
	info "Executing ${K8S_TEST_ENTRY}"
	if ! bats --show-output-of-passing-tests "${K8S_TEST_ENTRY}"; then
		tests_fail+=("${K8S_TEST_ENTRY}")
		[[ "${K8S_TEST_FAIL_FAST}" = "yes" ]] && break
	fi
done

[[ ${#tests_fail[@]} -ne 0 ]] && die "Tests FAILED from suites: ${tests_fail[*]}"

info "All tests SUCCEEDED"
