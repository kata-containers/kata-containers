#!/usr/bin/env bats
#
# Copyright (c) 2024 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

    get_pod_config_dir

    deployment_name="policy-redis-deployment"
    deployment_yaml="${pod_config_dir}/k8s-policy-deployment.yaml"

    # Add an appropriate policy to the correct YAML file.
    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
    add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
    auto_generate_policy "${policy_settings_dir}" "${deployment_yaml}"
}

@test "Successful deployment with auto-generated policy and container image volumes" {
    # Initiate deployment
    kubectl apply -f "${deployment_yaml}"

    # Wait for the deployment to be created
    cmd="kubectl rollout status --timeout=1s deployment/${deployment_name} | grep 'successfully rolled out'"
    info "Waiting for: ${cmd}"
    waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"
}

teardown() {
    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

    # Debugging information
    info "Deployment ${deployment_name}:"
    kubectl describe deployment "${deployment_name}"
    kubectl rollout status deployment/${deployment_name}

    # Clean-up
    kubectl delete deployment "${deployment_name}"

    delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
