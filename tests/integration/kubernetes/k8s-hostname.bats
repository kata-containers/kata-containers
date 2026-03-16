#!/usr/bin/env bats
#
# Copyright (c) 2024 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="test-pod-hostname"
	setup_common || die "setup_common failed"
	yaml_file="${pod_config_dir}/pod-hostname.yaml"

	expected_name=$pod_name

	# Add policy to yaml
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Validate Pod hostname" {
	# Create pod
	kubectl apply -f "${yaml_file}"

	kubectl wait --for jsonpath=status.phase=Succeeded --timeout=$timeout pod "$pod_name"

	# Validate the pod hostname
	result=$(kubectl logs $pod_name)
	[ "$pod_name" == "$result" ]
}

@test "test network performance with nydus" {
	curl_cmd='curl -I -s -o /dev/null \
-w "code=%{http_code} ip=%{remote_ip} dns=%{time_namelookup}s connect=%{time_connect}s tls=%{time_appconnect}s starttransfer=%{time_starttransfer}s total=%{time_total}s\n" \
https://ghcr.io/v2/dragonflyoss/image-service/alpine/blobs/sha256:12dba7d4fae4c70e1421021dd1ef3e8a1a4f1a9369074fc912a636dd2afdd640'

	kubectl apply -f "${yaml_file}"
	kubectl wait --for jsonpath=status.phase=Succeeded --timeout=$timeout pod "$pod_name"
	result=$(kubectl exec "$pod_name" -- sh -c "$curl_cmd")
	echo "$result"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
