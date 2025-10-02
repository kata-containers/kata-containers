#!/bin/bash
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

kubernetes_dir=$(dirname "$(readlink -f "$0")")
# shellcheck disable=SC1091 # import based on variable
source "${kubernetes_dir}/../../common.bash"

# Enable NVRC trace logging for NVIDIA GPU runtime
enable_nvrc_trace() {
	if [[ ${RUNTIME_CLASS_NAME:-kata-qemu-nvidia-gpu} == "kata-qemu-nvidia-gpu" ]]; then
		config_file="/opt/kata/share/defaults/kata-containers/configuration-qemu-nvidia-gpu.toml"
	fi
	if ! grep -q "nvrc.log=trace" "${config_file}"; then
		sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 nvrc.log=trace"/g' "${config_file}"
	fi
}

cleanup() {
	true
}

trap cleanup EXIT

# Setting to "yes" enables fail fast, stopping execution at the first failed test.
K8S_TEST_FAIL_FAST="${K8S_TEST_FAIL_FAST:-no}"

# Enable NVRC trace logging by default for NVIDIA GPU tests
ENABLE_NVRC_TRACE="${ENABLE_NVRC_TRACE:-true}"

if [ -n "${K8S_TEST_NV:-}" ]; then
	K8S_TEST_NV=($K8S_TEST_NV)
else
	K8S_TEST_NV=("k8s-nvidia-cuda.bats" \
		"k8s-nvidia-nim.bats")
fi

ensure_yq

if [[ "${ENABLE_NVRC_TRACE:-true}" == "true" ]]; then
	enable_nvrc_trace
fi

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
