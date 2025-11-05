#!/usr/bin/env bats
#
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

RUNTIME_CLASS_NAME=${RUNTIME_CLASS_NAME:-kata-qemu-nvidia-gpu}
export RUNTIME_CLASS_NAME

POD_NAME_CUDA="cuda-vectoradd-kata"
export POD_NAME_CUDA

setup() {
    [ "${KATA_HYPERVISOR}" = "qemu-nvidia-gpu-snp" ] && skip "The CC version of the test is under development"

    setup_common
    get_pod_config_dir

    pod_name="${POD_NAME_CUDA}"
    pod_yaml_in="${pod_config_dir}/nvidia-cuda-vectoradd.yaml.in"
    pod_yaml="${pod_config_dir}/nvidia-cuda-vectoradd.yaml"

    # Substitute environment variables in the YAML template
    envsubst < "${pod_yaml_in}" > "${pod_yaml}"

}

@test "CUDA Vector Addition Test" {
    [ "${KATA_HYPERVISOR}" = "qemu-nvidia-gpu-snp" ] && skip "The CC version of the test is under development"

    # Create the CUDA pod
    kubectl apply -f "${pod_yaml}"

    # Wait for pod to complete successfully
    kubectl wait --for=jsonpath='{.status.phase}'=Succeeded --timeout=300s pod "${pod_name}"

    # Get and verify the output contains expected CUDA success message
    output=$(kubectl logs "${pod_name}")
    echo "# CUDA Vector Add Output: ${output}" >&3

    # The CUDA vectoradd sample outputs "Test PASSED" on success
    [[ "${output}" =~ "Test PASSED" ]]
}

teardown() {
    [ "${KATA_HYPERVISOR}" = "qemu-nvidia-gpu-snp" ] && skip "The CC version of the test is under development"

    # Debugging information
    echo "=== CUDA vectoradd Pod Logs ==="
    kubectl logs "${pod_name}" || true

    teardown_common "${node}" "${node_start_time:-}"
}
