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

    replication_name="policy-rc-test"
    app_name="policy-nginx-rc"

    get_pod_config_dir

    correct_yaml="${pod_config_dir}/test-k8s-policy-rc.yaml"
    incorrect_yaml="${pod_config_dir}/test-k8s-policy-rc-incorrect.yaml"

    # Save some time by executing genpolicy a single time.
    if [ "${BATS_TEST_NUMBER}" == "1" ]; then
        # Create the correct yaml file
        nginx_version="${docker_images_nginx_version}"
        nginx_image="nginx:$nginx_version"

        sed -e "s/\${nginx_version}/${nginx_image}/" \
            "${pod_config_dir}/k8s-policy-rc.yaml" > "${correct_yaml}"

        # Add policy to the correct yaml file
        policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
        add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
        auto_generate_policy "${policy_settings_dir}" "${correct_yaml}"
    fi

    # Start each test case with a copy of the correct yaml file.
    cp "${correct_yaml}" "${incorrect_yaml}"

    # teardown() parses this string for pod names and prints the output of "kubectl describe" for these pods.
    declare -a launched_pods=()
}

# Common function for all test cases from this bats script.
test_rc_policy() {
    expect_denied_create_container=$1

    # Create replication controller
    if [ "${expect_denied_create_container}" = "true" ]; then
        kubectl create -f "${incorrect_yaml}"
    else
        kubectl create -f "${correct_yaml}"
    fi

    # Check replication controller
    local cmd="kubectl describe rc ${replication_name} | grep replication-controller"
    info "Waiting for: ${cmd}"
    waitForProcess "$wait_time" "$sleep_time" "$cmd"

    number_of_replicas=$(kubectl get rc ${replication_name} \
        --output=jsonpath='{.spec.replicas}')
    [ "${number_of_replicas}" -gt 0 ]

    # The replicas pods can be in running, waiting, succeeded or failed
    # status. We need them all on running state before proceeding.
    cmd="kubectl describe rc ${replication_name}"
    cmd+=" | grep \"Pods Status\" | grep \"${number_of_replicas} Running\""
    info "Waiting for: ${cmd}"
    waitForProcess "$wait_time" "$sleep_time" "$cmd"

    # Check that the number of pods created for the replication controller
    # is equal to the number of replicas that we defined.
    launched_pods=($(kubectl get pods "--selector=app=${app_name}" \
        --output=jsonpath={.items..metadata.name}))
    [ "${#launched_pods[@]}" -eq "${number_of_replicas}" ]

    # Check pod creation
    for pod_name in ${launched_pods[@]}; do
        if [ "${expect_denied_create_container}" = "true" ]; then
            wait_for_blocked_request "CreateContainerRequest" "${pod_name}"
        else
            cmd="kubectl wait --for=condition=Ready --timeout=${timeout} pod ${pod_name}"
            info "Waiting for: ${cmd}"
            waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"
        fi
    done
}

@test "Successful replication controller with auto-generated policy" {
    test_rc_policy false
}

@test "Policy failure: unexpected container command" {
    # Changing the template spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
      '.spec.template.spec.containers[0].command += ["ls"]' \
      "${incorrect_yaml}"

    test_rc_policy true
}

@test "Policy failure: unexpected volume mountPath" {
    # Changing the template spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
      '.spec.template.spec.containers[0].volumeMounts[0].mountPath = "/host/unexpected"' \
      "${incorrect_yaml}"

    test_rc_policy true
}

@test "Policy failure: unexpected host device mapping" {
    # Changing the template spec after generating its policy will cause CreateContainer to be denied.
  yq -i \
      '.spec.template.spec.containers[0].volumeMounts += [{"mountPath": "/dev/ttyS0", "name": "dev-ttys0"}]' \
      "${incorrect_yaml}"

  yq -i \
      '.spec.template.spec.volumes += [{"name": "dev-ttys0", "hostPath": {"path": "/dev/ttyS0"}}]' \
      "${incorrect_yaml}"

    test_rc_policy true
}

@test "Policy failure: unexpected securityContext.allowPrivilegeEscalation" {
    # Changing the template spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
      '.spec.template.spec.containers[0].securityContext.allowPrivilegeEscalation = false' \
      "${incorrect_yaml}"

    test_rc_policy true
}

@test "Policy failure: unexpected capability" {
    # Changing the template spec after generating its policy will cause CreateContainer to be denied.
    yq -i \
      '.spec.template.spec.containers[0].securityContext.capabilities.add += ["CAP_SYS_CHROOT"]' \
      "${incorrect_yaml}"

    test_rc_policy true
}

teardown() {
    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

    # Debugging information
    kubectl describe rc "${replication_name}"

    for pod_name in ${launched_pods[@]}; do
        info "Pod ${pod_name}:"

        # Don't print the "Message:" line because it contains a truncated policy log.
        kubectl describe pod "${pod_name}" | grep -v "Message:"
    done

    # Clean-up
    kubectl delete rc "${replication_name}"

    info "Deleting ${incorrect_yaml}"
    rm -f "${incorrect_yaml}"

    if [ "${BATS_TEST_NUMBER}" == "1" ]; then
        delete_tmp_policy_settings_dir "${policy_settings_dir}"
    fi
}
