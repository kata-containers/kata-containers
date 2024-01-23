#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="custom-dns-test"
	file_name="/etc/resolv.conf"
	get_pod_config_dir
	yaml_file="${pod_config_dir}/pod-custom-dns.yaml"
}

@test "Check custom dns" {
	# TODO: disabled due to #8850
	# auto_generate_policy "${yaml_file}"

	# Create the pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Check dns config at /etc/resolv.conf
	kubectl exec "$pod_name" -- cat "$file_name" | grep -q "nameserver 1.2.3.4"
	kubectl exec "$pod_name" -- cat "$file_name" | grep -q "search dns.test.search"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
