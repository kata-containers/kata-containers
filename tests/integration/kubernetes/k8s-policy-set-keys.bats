#!/usr/bin/env bats
#
# Copyright (c) 2023 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	policy_tests_enabled || skip "Policy tests are disabled."

	get_pod_config_dir
	pod_name="set-keys-test"
	pod_yaml="${pod_config_dir}/k8s-policy-set-keys.yaml"
	set_keys_policy=$(base64 -w 0 "${pod_config_dir}/k8s-policy-set-keys.rego")
}

@test "Set guest keys using policy" {
	yq write -i "${pod_yaml}" \
		'metadata.annotations."io.katacontainers.config.agent.policy"' \
		"${set_keys_policy}"

	# Create the pod
	kubectl create -f "${pod_yaml}"

	# Wait for pod to start
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Obtain the keys from the policy by querying the OPA service
	my_test_data="http://localhost:8181/v1/data/agent_policy/my_test_data"
	kubectl exec "$pod_name" -- wget -O - "$my_test_data/default/key/ssh-demo" | grep "{\"result\":\"HUlOu8NWz8si11OZUzUJMnjiq/iZyHBJZMSD3BaqgMc=\"}"
	kubectl exec "$pod_name" -- wget -O - "$my_test_data/default/key/enabled" | grep "{\"result\":false}"
	kubectl exec "$pod_name" -- wget -O - "$my_test_data/key1" | grep "{\"result\":\[\"abc\",\"9876\",\"xyz\"\]}"
	kubectl exec "$pod_name" -- wget -O - "$my_test_data/key2" | grep "{\"result\":45}"
}

teardown() {
	policy_tests_enabled || skip "Policy tests are disabled."

	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
