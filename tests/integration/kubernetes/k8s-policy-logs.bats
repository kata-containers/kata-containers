#!/usr/bin/env bats
# Copyright (c) 2025 Microsoft Corporation
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled"

    setup_common
    get_pod_config_dir

    pod_name="test-pod-hostname"
    yaml_file="${pod_config_dir}/pod-hostname.yaml"
    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

    # Assert that ReadStreamRequest is indeed blocked.
    [[ "$(jq .request_defaults.ReadStreamRequest "${policy_settings_dir}/genpolicy-settings.json")" == "false" ]]

    auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Logs empty when ReadStreamRequest is blocked" {
    kubectl apply -f "${yaml_file}"
    kubectl wait --for jsonpath=status.phase=Succeeded --timeout=$timeout pod "$pod_name"

    # Verify that (1) the logs are empty and (2) the container does not deadlock.
    [[ "$(kubectl logs "${pod_name}")" == "" ]]
}

teardown() {
    auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled"
    teardown_common "${node}" "${node_start_time:-}" "${policy_settings_dir}"
}
