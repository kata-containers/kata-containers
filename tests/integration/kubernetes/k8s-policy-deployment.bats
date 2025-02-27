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
    correct_deployment_yaml="${pod_config_dir}/k8s-policy-deployment.yaml"

    # Save some time by executing genpolicy a single time.
    if [ "${BATS_TEST_NUMBER}" == "1" ]; then
        # Add an appropriate policy to the correct YAML file.
        policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
        add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
        auto_generate_policy "${policy_settings_dir}" "${correct_deployment_yaml}"
    fi

    # Start each test case with a copy of the correct yaml file.
    incorrect_deployment_yaml="${pod_config_dir}/k8s-policy-deployment-incorrect.yaml"
    cp "${correct_deployment_yaml}" "${incorrect_deployment_yaml}"
}

@test "Successful deployment with auto-generated policy and container image volumes" {
    # Initiate deployment
    kubectl apply -f "${correct_deployment_yaml}"

    # Wait for the deployment to be created
    cmd="kubectl rollout status --timeout=1s deployment/${deployment_name} | grep 'successfully rolled out'"
    info "Waiting for: ${cmd}"
    waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"
}

test_deployment_policy_error() {
    # Initiate deployment
    kubectl apply -f "${incorrect_deployment_yaml}"

    # Wait for the deployment pod to fail
    wait_for_blocked_request "CreateContainerRequest" "${deployment_name}"
}

@test "Policy failure: unexpected UID = 0" {
    # Change the pod UID to 0 after the policy has been generated using a different
    # runAsUser value. The policy would use UID = 0 by default, if there weren't
    # a different runAsUser value in the YAML file.
    yq -i \
        '.spec.template.spec.securityContext.runAsUser = 0' \
        "${incorrect_deployment_yaml}"

    test_deployment_policy_error
}

teardown() {
    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

    # Pod debugging information. Don't print the "Message:" line because it contains a truncated policy log.
    info "Pod ${deployment_name}:"
    kubectl describe pod "${deployment_name}" | grep -v "Message:"

    # Deployment debugging information. The --watch=false argument makes "kubectl rollout status"
    # return instead of waiting for a possibly failed deployment to complete.
    info "Deployment ${deployment_name}:"
    kubectl describe deployment "${deployment_name}"
    kubectl rollout status deployment/${deployment_name} --watch=false

    # Clean-up
    kubectl delete deployment "${deployment_name}"

    delete_tmp_policy_settings_dir "${policy_settings_dir}"
    rm -f "${incorrect_deployment_yaml}"
}
