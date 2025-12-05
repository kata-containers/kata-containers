#!/usr/bin/env bats
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

RUNTIME_CLASS_NAME=${RUNTIME_CLASS_NAME:-kata-qemu-nvidia-gpu}
export RUNTIME_CLASS_NAME

KATA_HYPERVISOR=${KATA_HYPERVISOR:-${RUNTIME_CLASS_NAME#kata-}}
export KATA_HYPERVISOR

# TODO: Replace with is_confidential_gpu_hardware() once available
TEE=false
[[ "${RUNTIME_CLASS_NAME}" = "kata-qemu-nvidia-gpu-snp" ]] && TEE=true
[[ "${RUNTIME_CLASS_NAME}" = "kata-qemu-nvidia-gpu-tdx" ]] && TEE=true
export TEE

export POD_NAME_CUDA="nvidia-cuda-vectoradd"

POD_WAIT_TIMEOUT=${POD_WAIT_TIMEOUT:-300s}
export POD_WAIT_TIMEOUT

setup() {
    setup_common
    get_pod_config_dir

    pod_yaml_in="${pod_config_dir}/${POD_NAME_CUDA}.yaml.in"
    pod_yaml="${pod_config_dir}/${POD_NAME_CUDA}.yaml"

    envsubst < "${pod_yaml_in}" > "${pod_yaml}"
    add_allow_all_policy_to_yaml "${pod_yaml}"

    if [ "${TEE}" = "true" ]; then
        kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
        kernel_params_value="nvrc.smi.srs=1"
        set_metadata_annotation "${pod_yaml}" \
            "${kernel_params_annotation}" \
            "${kernel_params_value}"
    fi

    setup_cdi_override_for_nvidia_gpu_snp
}

@test "CUDA Vector Addition Test" {
    # Create the CUDA pod
    kubectl apply -f "${pod_yaml}"

    # Wait for pod to complete successfully (with retry)
    kubectl wait --for=jsonpath='{.status.phase}'=Succeeded --timeout="${POD_WAIT_TIMEOUT}" pod "${POD_NAME_CUDA}"

    # Get and verify the output contains expected CUDA success message
    kubectl logs "${POD_NAME_CUDA}"
    output=$(kubectl logs "${POD_NAME_CUDA}")
    echo "# CUDA Vector Add Output: ${output}" >&3

    # The CUDA vectoradd sample outputs "Test PASSED" on success
    [[ "${output}" =~ "Test PASSED" ]]
}

teardown() {
    # Debugging information
    echo "=== CUDA vectoradd Pod Logs ==="
    kubectl logs "${POD_NAME_CUDA}" || true

    teardown_cdi_override_for_nvidia_gpu_snp

    teardown_common "${node}" "${node_start_time:-}"
}
