#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Kubelet writes each pod's /etc/hosts, and some of what it puts there, the
# pod's FQDN and its hostAliases, exists nowhere else a pod could look it up.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="test-pod-hosts"
	setup_common || die "setup_common failed"
	yaml_file="${pod_config_dir}/pod-hosts.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Pod /etc/hosts carries hostname, FQDN and hostAliases" {
	kubectl apply -f "${yaml_file}"
	kubectl wait --for jsonpath=status.phase=Succeeded --timeout="${timeout}" pod "${pod_name}"

	hosts="$(kubectl logs "${pod_name}")"
	echo "${hosts}"

	pod_ip="$(kubectl get pod "${pod_name}" -o jsonpath='{.status.podIP}')"
	namespace="$(kubectl get pod "${pod_name}" -o jsonpath='{.metadata.namespace}')"

	grep -qP '^127\.0\.0\.1\tlocalhost$' <<< "${hosts}"
	grep -qP "^${pod_ip//./\\.}\thosts-probe\.hosts-sub\.${namespace}\.svc\.[^\t]+\thosts-probe$" <<< "${hosts}"
	grep -qP '^# Entries added by HostAliases\.$' <<< "${hosts}"
	grep -qP '^10\.99\.99\.99\tfoo\.local\tbar\.local$' <<< "${hosts}"
	grep -qP '^10\.99\.99\.100\tbaz\.remote$' <<< "${hosts}"
}

teardown() {
	kubectl describe "pod/${pod_name}"
	kubectl delete pod "${pod_name}" --ignore-not-found=true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
