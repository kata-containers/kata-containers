#!/usr/bin/env bats
#
# Copyright (c) 2024 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."
    setup_common
    get_pod_config_dir

    job_name="policy-job"
    correct_yaml="${pod_config_dir}/k8s-policy-job.yaml"
    incorrect_yaml="${pod_config_dir}/k8s-policy-job-incorrect.yaml"
    set_node "${correct_yaml}" "${node}"
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
    abort_cmd="kubectl describe pod ${job_name} | grep \"CreateContainerRequest is blocked by policy\""
    info "Waiting ${wait_time}s with sleep ${sleep_time}s for: ${cmd}. Abort if: ${abort_cmd}."
    waitForCmdWithAbortCmd "${wait_time}" "${sleep_time}" "${cmd}" "${abort_cmd}"

    # Wait for the job to complete
    cmd="kubectl get pods -o jsonpath='{.items[*].status.phase}' | grep Succeeded"
    info "Waiting ${wait_time}s with sleep ${sleep_time}s for: ${cmd}. Abort if: ${abort_cmd}."
    waitForCmdWithAbortCmd "${wait_time}" "${sleep_time}" "${cmd}" "${abort_cmd}"
}

# Common function for all test cases that expect CreateContainer to be blocked by policy.
test_job_policy_error() {
    local max_attempts=5
    local attempt_num
    local sleep_between_attempts=5

    for attempt_num in $(seq 1 "${max_attempts}"); do
        info "Starting attempt #${attempt_num}"

        # Cleanup possible previous resources
        kubectl delete -f "${incorrect_yaml}" --ignore-not-found=true --now --timeout=120s

        # 1. Apply Job
        kubectl apply -f "${incorrect_yaml}"
        if [ $? -ne 0 ]; then
            warn "Failed to apply Job. Retrying..."
            continue
        fi

        # 2. Wait for Job creation event
        cmd="kubectl describe job ${job_name} | grep SuccessfulCreate"
        info "Waiting for: ${cmd}"

        run waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"
        if [ "$status" -ne 0 ]; then
            warn "waitForProcess FAILED on attempt #${attempt_num}"
            continue
        fi

        # 3. Get pod list
        pod_names=$(kubectl get pods "--selector=job-name=${job_name}" --output=jsonpath='{.items[*].metadata.name}')
        info "pod_names: ${pod_names}"

        if [ -z "${pod_names}" ]; then
            warn "No pods found for job. Retrying..."
            continue
        fi

        # 4. Check each pod for blocked CreateContainerRequest
        for pod_name in ${pod_names[@]}; do
            info "Checking pod: ${pod_name}"

            run wait_for_blocked_request "CreateContainerRequest" "${pod_name}"
            if [ "$status" -eq 0 ]; then
                info "wait_for_blocked_request succeeded for pod ${pod_name} on attempt #${attempt_num}"
                return 0
            else
                warn "wait_for_blocked_request FAILED for pod ${pod_name} on attempt #${attempt_num}"
                # We break pod loop, but the attempt will continue
                break
            fi
        done

        # Retry if not last attempt
        if [ "${attempt_num}" -lt "${max_attempts}" ]; then
            info "Retrying in ${sleep_between_attempts} seconds..."
            sleep "${sleep_between_attempts}"
        fi
    done

    error "Test failed after ${max_attempts} attempts."
    return 1
}

@test "Policy failure: unexpected environment variable" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        '.spec.template.spec.containers[0].env += [{"name": "unexpected_variable", "value": "unexpected_value"}]' \
        "${incorrect_yaml}"

    test_job_policy_error
    test_result=$?
    [ "${test_result}" -eq 0 ]
}

@test "Policy failure: unexpected command line argument" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        '.spec.template.spec.containers[0].args += ["unexpected_arg"]' \
        "${incorrect_yaml}"

    test_job_policy_error
    test_result=$?
    [ "${test_result}" -eq 0 ]
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
    test_result=$?
    [ "${test_result}" -eq 0 ]
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
    test_result=$?
    [ "${test_result}" -eq 0 ]
}

@test "Policy failure: unexpected readOnlyRootFilesystem" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        ".spec.template.spec.containers[0].securityContext.readOnlyRootFilesystem = false" \
        "${incorrect_yaml}"

    test_job_policy_error
    test_result=$?
    [ "${test_result}" -eq 0 ]
}

@test "Policy failure: unexpected UID = 222" {
    # Changing the job spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
        '.spec.template.spec.securityContext.runAsUser = 222' \
        "${incorrect_yaml}"

    test_job_policy_error
    test_result=$?
    [ "${test_result}" -eq 0 ]
}

teardown() {
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

    teardown_common "${node}" "${node_start_time:-}"
}
