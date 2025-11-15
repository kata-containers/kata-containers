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

# TODO: Replace with is_confidential_gpu_hardware() once available
TEE=false
[[ "${RUNTIME_CLASS_NAME}" = "kata-qemu-nvidia-gpu-snp" ]] && TEE=true
[[ "${RUNTIME_CLASS_NAME}" = "kata-qemu-nvidia-gpu-tdx" ]] && TEE=true
export TEE

POD_NAME_CUDA="cuda-vectoradd-kata"
export POD_NAME_CUDA

POD_WAIT_TIMEOUT=${POD_WAIT_TIMEOUT:-300s}
export POD_WAIT_TIMEOUT

setup() {
    setup_common
    get_pod_config_dir

    pod_name="${POD_NAME_CUDA}"
    pod_yaml_in="${pod_config_dir}/nvidia-cuda-vectoradd.yaml.in"
    pod_yaml="${pod_config_dir}/nvidia-cuda-vectoradd.yaml"

    # Substitute environment variables in the YAML template
    envsubst < "${pod_yaml_in}" > "${pod_yaml}"

    # For TEE environments, set up KBS and add kernel params annotation
    if [ "${TEE}" = "true" ]; then
        export CC_KBS_ADDR="$(kbs_k8s_svc_http_addr)"

        # TODO: Replace with kbs_set_gpu_attestation_policy() once available
        kbs_set_allow_all_resources

        # kernel parameters for attestation
        kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
        kernel_params_value="agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
        set_metadata_annotation "${pod_yaml}" \
            "${kernel_params_annotation}" \
            "${kernel_params_value}"
    fi
}

@test "CUDA Vector Addition Test" {
    # Create the CUDA pod
    kubectl apply -f "${pod_yaml}"

    # Wait for pod to complete successfully
    kubectl wait --for=jsonpath='{.status.phase}'=Succeeded --timeout="${POD_WAIT_TIMEOUT}" pod "${pod_name}"

    # Get and verify the output contains expected CUDA success message
    output=$(kubectl logs "${pod_name}")
    echo "# CUDA Vector Add Output: ${output}" >&3

    # The CUDA vectoradd sample outputs "Test PASSED" on success
    [[ "${output}" =~ "Test PASSED" ]]
}

teardown() {
    # Debugging information
    echo "=== CUDA vectoradd Pod Logs ==="
    kubectl logs "${pod_name}" || true

    teardown_common "${node}" "${node_start_time:-}"
}
