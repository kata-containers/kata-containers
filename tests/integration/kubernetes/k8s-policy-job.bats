#!/usr/bin/env bats
#
# Copyright (c) 2024 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
    if [ "${KATA_HYPERVISOR}" == "qemu-coco-dev" ]; then
        skip "Test not stable on qemu-coco-dev. See issue #10616"
    fi

    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

    get_pod_config_dir

    job_name="policy-job"
    correct_yaml="${pod_config_dir}/k8s-policy-job.yaml"
    incorrect_yaml="${pod_config_dir}/k8s-policy-job-incorrect.yaml"

    # Save some time by executing genpolicy a single time.
    if [ "${BATS_TEST_NUMBER}" == "1" ]; then
        # Add an appropriate policy to the correct YAML file.
        policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
        add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
        auto_generate_policy "${policy_settings_dir}" "${correct_yaml}"
    fi

    # Start each test case with a copy of the correct yaml file.
    cp "${correct_yaml}" "${incorrect_yaml}"

    # teardown() parses this string for pod names and prints the output of "kubectl describe" for these pods.
    pod_names=""
}

@test "Successful job with auto-generated policy" {
    # Initiate job creation
    kubectl apply -f "${correct_yaml}"

    # Wait for the job to be created
    cmd="kubectl describe job ${job_name} | grep SuccessfulCreate"
    info "Waiting for: ${cmd}"
    waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

    # Wait for the job to complete
    cmd="kubectl get pods -o jsonpath='{.items[*].status.phase}' | grep Succeeded"
    info "Waiting for: ${cmd}"
    waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"
}

# Common function for all test cases that expect CreateContainer to be blocked by policy.
test_job_policy_error() {
    # Initiate job creation
    kubectl apply -f "${incorrect_yaml}"

    # Wait for the job to be created
    cmd="kubectl describe job ${job_name} | grep SuccessfulCreate"
    info "Waiting for: ${cmd}"
    waitForProcess "${wait_time}" "${sleep_time}" "${cmd}" || return 1

    # List the pods that belong to the job
    pod_names=$(kubectl get pods "--selector=job-name=${job_name}" --output=jsonpath='{.items[*].metadata.name}')
    info "pod_names: ${pod_names}"

    # CreateContainerRequest must have been denied by the policy.
    for pod_name in ${pod_names[@]}; do
        wait_for_blocked_request "CreateContainerRequest" "${pod_name}" || return 1
    done
}

@test "Policy failure: unexpected environment variable" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        '.spec.template.spec.containers[0].env += [{"name": "unexpected_variable", "value": "unexpected_value"}]' \
        "${incorrect_yaml}"

    test_job_policy_error
}

@test "Policy failure: unexpected command line argument" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        '.spec.template.spec.containers[0].args += ["unexpected_arg"]' \
        "${incorrect_yaml}"

    test_job_policy_error
}

@test "Policy failure: unexpected emptyDir volume" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        '.spec.template.spec.containers[0].volumeMounts += [{"mountPath": "/unexpected1", "name": "unexpected-volume1"}]' \
        "${incorrect_yaml}"

    yq -i \
        '.spec.template.spec.volumes += [{"name": "unexpected-volume1", "emptyDir": {"medium": "Memory", "sizeLimit": "50M"}}]' \
        "${incorrect_yaml}"

    test_job_policy_error
}

@test "Policy failure: unexpected projected volume" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        '.spec.template.spec.containers[0].volumeMounts += [{"mountPath": "/test-volume", "name": "test-volume", "readOnly": true}]' \
        "${incorrect_yaml}"

    yq -i '
      .spec.template.spec.volumes += [{
        "name": "test-volume",
        "projected": {
          "defaultMode": 420,
          "sources": [{
            "serviceAccountToken": {
              "expirationSeconds": 3600,
              "path": "token"
            }
          }]
        }
      }]
    ' "${incorrect_yaml}"

    test_job_policy_error
}

@test "Policy failure: unexpected readOnlyRootFilesystem" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        ".spec.template.spec.containers[0].securityContext.readOnlyRootFilesystem = false" \
        "${incorrect_yaml}"

    test_job_policy_error
}

@test "Policy failure: unexpected UID = 222" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        '.spec.template.spec.securityContext.runAsUser = 222' \
        "${incorrect_yaml}"

    test_job_policy_error
}

teardown() {
    if [ "${KATA_HYPERVISOR}" == "qemu-coco-dev" ]; then
        skip "Test not stable on qemu-coco-dev. See issue #10616"
    fi

    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

    # Debugging information
    for pod_name in ${pod_names[@]}; do
        info "Pod ${pod_name}:"

        # Don't print the "Message:" line because it contains a truncated policy log.
        kubectl describe pod "${pod_name}" | grep -v "Message:"
    done

    info "Job ${job_name}:"
    kubectl describe job "${job_name}"

    # Clean-up
    kubectl delete job "${job_name}"

    info "Deleting ${incorrect_yaml}"
    rm -f "${incorrect_yaml}"

    if [ "${BATS_TEST_NUMBER}" == "1" ]; then
        delete_tmp_policy_settings_dir "${policy_settings_dir}"
    fi
}
