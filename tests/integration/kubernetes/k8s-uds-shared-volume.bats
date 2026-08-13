#!/usr/bin/env bats
#
# Copyright (c) NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="test-uds-shared-volume"
	server_container="uds-server"
	client_container="uds-client"
	curl_command=(curl --silent --show-error --unix-socket /uds/echo.sock "http://localhost/")

	setup_common || die "setup_common failed"

	yaml_file="${pod_config_dir}/test-pod-uds-shared-volume.yaml"
	set_nginx_image "${pod_config_dir}/pod-uds-shared-volume.yaml" "${yaml_file}"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${policy_settings_dir}" "${curl_command[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
}

@test "Containers communicate over a unix domain socket on a shared emptyDir volume" {
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	kubectl create -f "${yaml_file}"

	# The server's readiness probe only passes once it has bound the socket on
	# the volume, so waiting for the pod covers the socket creation.
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	container_exec "${pod_name}" "${client_container}" "${curl_command[@]}" \
		| grep "uds reply from the shared emptyDir"
}

teardown() {
	# A volume that cannot hold a socket keeps the pod NotReady, and only the
	# server's log says why, as an nginx bind() error.
	[[ -n "${BATS_TEST_COMPLETED:-}" ]] || kubectl logs "${pod_name}" -c "${server_container}" || true

	[[ -z "${yaml_file:-}" ]] || rm -f "${yaml_file}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
