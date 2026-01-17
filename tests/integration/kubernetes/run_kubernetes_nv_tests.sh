#!/bin/bash
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

kubernetes_dir=$(dirname "$(readlink -f "$0")")
# shellcheck disable=SC1091 # import based on variable
source "${kubernetes_dir}/../../common.bash"

# Enable NVRC trace logging for NVIDIA GPU runtime
enable_nvrc_trace() {
	local config_file="/opt/kata/share/defaults/kata-containers/configuration-${KATA_HYPERVISOR}.toml"

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

if [[ -n "${K8S_TEST_NV:-}" ]]; then
	mapfile -d " " -t K8S_TEST_NV <<< "${K8S_TEST_NV}"
else
	K8S_TEST_NV=("k8s-confidential-attestation.bats" \
		"k8s-nvidia-cuda.bats" \
		"k8s-nvidia-nim.bats")
fi

SUPPORTED_HYPERVISORS=("qemu-nvidia-gpu" "qemu-nvidia-gpu-snp" "qemu-nvidia-gpu-tdx")
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-nvidia-gpu}"
# shellcheck disable=SC2076 # intentionally use literal string matching
if [[ ! " ${SUPPORTED_HYPERVISORS[*]} " =~ " ${KATA_HYPERVISOR} " ]]; then
	die "Unsupported KATA_HYPERVISOR=${KATA_HYPERVISOR}. Must be one of: ${SUPPORTED_HYPERVISORS[*]}"
fi

ensure_yq

if [[ "${ENABLE_NVRC_TRACE:-true}" == "true" ]]; then
	enable_nvrc_trace
fi

report_dir="${kubernetes_dir}/reports/$(date +'%F-%T')"
mkdir -p "${report_dir}"

info "Running tests with bats version: $(bats --version). Save outputs to ${report_dir}"

tests_fail=()
for K8S_TEST_ENTRY in "${K8S_TEST_NV[@]}"
do
	K8S_TEST_ENTRY=$(echo "${K8S_TEST_ENTRY}" | tr -d '[:space:][:cntrl:]')
	time info "$(kubectl get pods --all-namespaces 2>&1)"
	info "Executing ${K8S_TEST_ENTRY}"
	# Output file will be prefixed with "ok" or "not_ok" based on the result
	out_file="${report_dir}/${K8S_TEST_ENTRY}.out"
	if ! bats --timing --show-output-of-passing-tests "${K8S_TEST_ENTRY}" | tee "${out_file}"; then
		tests_fail+=("${K8S_TEST_ENTRY}")
		mv "${out_file}" "$(dirname "${out_file}")/not_ok-$(basename "${out_file}")"
		[[ "${K8S_TEST_FAIL_FAST}" = "yes" ]] && break
	else
		mv "${out_file}" "$(dirname "${out_file}")/ok-$(basename "${out_file}")"
	fi
done

[[ ${#tests_fail[@]} -ne 0 ]] && die "Tests FAILED from suites: ${tests_fail[*]}"

info "All tests SUCCEEDED"
