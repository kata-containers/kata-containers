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
    local config_file=""
    if [[ ${RUNTIME_CLASS_NAME} == "kata-qemu-nvidia-gpu" ]]; then
        config_file="/opt/kata/share/defaults/kata-containers/configuration-qemu-nvidia-gpu.toml"
    elif [[ ${RUNTIME_CLASS_NAME} == "kata-qemu-nvidia-gpu-snp" ]]; then
        config_file="/opt/kata/share/defaults/kata-containers/configuration-qemu-nvidia-gpu-snp.toml"
    fi
    sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 nvrc.log=trace"/g' "${config_file}"
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

# KATA_HYPERVISOR is set in the CI workflow yaml file, and can be set by the user executing CI locally
if [ -n "${KATA_HYPERVISOR:-}" ]; then
	export RUNTIME_CLASS_NAME="kata-${KATA_HYPERVISOR}"
	info "Set RUNTIME_CLASS_NAME=${RUNTIME_CLASS_NAME} from KATA_HYPERVISOR=${KATA_HYPERVISOR}"
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
