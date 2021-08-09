#!/usr/bin/env bats
#
# Copyright (c) 2021 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
        export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
        direct_pod_name="direct-mount-test-0"
        indirect_pod_name="host-mount-test-0"
        get_pod_config_dir
}

@test "Check direct-assignment behavior" {
        # Create stateful sets:
        kubectl create -f "${pod_config_dir}/pod-direct-assign.yaml"
        kubectl create -f "${pod_config_dir}/pod-host-mount.yaml"

        # Check direct-assign pod creation
        kubectl wait --for=condition=Ready --timeout=$timeout pod "$direct_pod_name"

        # Verify the direct-assign mount is ext4:
        kubectl exec -it "$direct_pod_name" -- sh -c "mount | grep data" | grep -q ext4

        # Check host mounted PVC pod creation:
        kubectl wait --for=condition=Ready --timeout=$timeout pod "$indirect_pod_name"
        # Verify the volume is mounted using virtiofs:
        kubectl exec -it "$indirect_pod_name" -- sh -c "mount | grep data" | grep -q virtiofs

}

teardown() {
        kubectl delete -f "${pod_config_dir}/pod-direct-assign.yaml"
        kubectl delete -f "${pod_config_dir}/pod-host-mount.yaml"
        kubectl delete pvc testing-pvc-direct-mount-test-0 testing-pvc-host-mount-test-0
}
