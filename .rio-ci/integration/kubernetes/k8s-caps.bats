#!/usr/bin/env bats
#
# Copyright (c) 2021 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
        export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
        pod_name="pod-caps"
        get_pod_config_dir
        expected="CapInh.*445fb"
}

# Skip on aarch64 due to missing cpu hotplug related functionality.
@test "Check capabilities of pod" {
        # Create pod
        kubectl create -f "${pod_config_dir}/pod-caps.yaml"
        # Check pod creation
        kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

        # Verify expected capabilities for the running container. Add retry to ensure
        # that the container had time to execute:
        wait_time=5
        sleep_time=1
        cmd="kubectl logs $pod_name | grep -q $expected"
        waitForProcess "$wait_time" "$sleep_time" "$cmd"

        # Verify expected capabilities from exec context:
        kubectl exec -it "$pod_name" -- sh -c "cat /proc/self/status | grep Cap" | grep -q "$expected"
}

teardown() {
        # Debugging information
        kubectl describe "pod/$pod_name"
        kubectl delete pod "$pod_name"
}
