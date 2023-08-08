#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/lib.sh"

setup() {
	pod_name="custom-dns-test"
	file_name="/etc/resolv.conf"
	get_pod_config_dir
}

@test "Check custom dns" {
	# Create the pod
	create_pod_and_wait "${pod_config_dir}/pod-custom-dns.yaml" "$pod_name"

	# Check dns config at /etc/resolv.conf
	kubectl exec "$pod_name" -- cat "$file_name" | grep -q "nameserver 1.2.3.4"
	kubectl exec "$pod_name" -- cat "$file_name" | grep -q "search dns.test.search"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
