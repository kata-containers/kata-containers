#!/usr/bin/env bats
#
# Copyright (c) 2025 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Specific test for early closed stdio issue in kubectl exec via WebSocket

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {

	setup_common || die "setup_common failed"
	pod_name="wss-exec-pod"
	container_name="container"

	artifacts_dir="$(mktemp -d "${BATS_TEST_DIRNAME}/artifacts.XXXXXXXXXX")"

	test_yaml_file="${pod_config_dir}/test-k8s-pod-wss.yaml"
    cp "$pod_config_dir/k8s-pod-wss.yaml" "${test_yaml_file}"

	info "Using existing test pod YAML file at ${test_yaml_file}"
	# Use pre-existing test pod YAML with a simple busybox container a simple stdout/err script

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	# Allow the exec command used by the WebSocket tests: sh -e test.sh
	add_exec_to_policy_settings "${policy_settings_dir}" "sh" "-e" "test.sh"

	allowed_requests=(
		"CloseStdinRequest"
		"ReadStreamRequest"
		"WriteStreamRequest"
	)
	add_requests_to_policy_settings "${policy_settings_dir}" "${allowed_requests[@]}"

	auto_generate_policy "${policy_settings_dir}" "${test_yaml_file}"

	# Get current context's cluster & username
	CURRENT_USER=$(kubectl config view -o jsonpath="{.contexts[?(@.name==\"$(kubectl config current-context)\")].context.user}")
	CURRENT_CLUSTER=$(kubectl config view -o jsonpath="{.contexts[?(@.name==\"$(kubectl config current-context)\")].context.cluster}")
	CURRENT_NAMESPACE=$(kubectl config view -o jsonpath="{.contexts[?(@.name==\"$(kubectl config current-context)\")].context.namespace}")
	if [ -z "${CURRENT_NAMESPACE}" ]; then
		CURRENT_NAMESPACE="kata-containers-k8s-tests"
	fi

	# Get that user's client-key-data
	kubectl config view --raw -o jsonpath="{.users[?(@.name==\"${CURRENT_USER}\")].user.client-key-data}" | base64 --decode > "${artifacts_dir}/client.key"
	kubectl config view --raw -o jsonpath="{.users[?(@.name==\"${CURRENT_USER}\")].user.client-certificate-data}" | base64 --decode > "${artifacts_dir}/client.crt"
	kubectl config view --raw -o jsonpath="{.clusters[?(@.name==\"${CURRENT_CLUSTER}\")].cluster.certificate-authority-data}" | base64 --decode > "${artifacts_dir}/ca.crt"


	CLUSTER_URL=$(kubectl config view --raw -o jsonpath="{.clusters[?(@.name==\"${CURRENT_CLUSTER}\")].cluster.server}")
	CLUSTER_HOST_PORT="${CLUSTER_URL//https:\/\/}"
}

@test "TestWebsocketOutput" {

	# Create the pod
	kubectl create -f "${test_yaml_file}" -n "${CURRENT_NAMESPACE}"

	# Wait for ready pod
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}" -n "${CURRENT_NAMESPACE}"

	# Use curl to open WebSocket connection
	curl \
		-s --cacert "${artifacts_dir}/ca.crt" --cert "${artifacts_dir}/client.crt" --key "${artifacts_dir}/client.key" \
		--connect-timeout 30 \
		--max-time 30 \
		--http1.1 \
		--no-buffer \
		--header "Connection: Upgrade" \
		--header "Upgrade: websocket" \
		--header "Host: ${CLUSTER_HOST_PORT}" \
		--header "Origin: ${CLUSTER_URL}" \
		--header "Sec-WebSocket-Version: 13" \
		--header "Sec-WebSocket-Protocol: channel.k8s.io" \
		--header "Sec-WebSocket-Key: MDEyMzQ1Njc4OWFiY2RlZg==" \
		"https://${CLUSTER_HOST_PORT}/api/v1/namespaces/${CURRENT_NAMESPACE}/pods/${pod_name}/exec?stdout=true&stderr=true&command=sh&command=-e&command=test.sh" \
		> "${artifacts_dir}/wss-output.log"

	# Check for exactly 1 occurrence of each error1-error2-error3 and out1-out2-out3
	for i in 1 2 3; do
		error_count=$(LC_ALL="C" grep --text -c "error${i}" "${artifacts_dir}/wss-output.log" || true)
		if [[ "${error_count}" -ne 1 ]]; then
			info "✗ Test FAILED: Expected exactly 1 occurrence of 'error${i}', found ${error_count}" >&2
			return 1
		fi

		out_count=$(LC_ALL="C" grep --text -c "out${i}" "${artifacts_dir}/wss-output.log" || true)
		if [[ "${out_count}" -ne 1 ]]; then
			info "✗ Test FAILED: Expected exactly 1 occurrence of 'out${i}', found ${out_count}" >&2
			return 1
		fi
	done
}

@test "TestWebsocketOutputWithStdin" {

	# Create the pod
	kubectl create -f "${test_yaml_file}" -n "${CURRENT_NAMESPACE}"

	# Wait for ready pod
	kubectl wait --for=condition=Ready --timeout=$timeout pod "${pod_name}" -n "${CURRENT_NAMESPACE}"

	# Use curl to open WebSocket connection with stdin
	curl \
		-s --cacert "${artifacts_dir}/ca.crt" --cert "${artifacts_dir}/client.crt" --key "${artifacts_dir}/client.key" \
		--connect-timeout 30 \
		--max-time 30 \
		--http1.1 \
		--no-buffer \
		--header "Connection: Upgrade" \
		--header "Upgrade: websocket" \
		--header "Host: ${CLUSTER_HOST_PORT}" \
		--header "Origin: ${CLUSTER_URL}" \
		--header "Sec-WebSocket-Version: 13" \
		--header "Sec-WebSocket-Protocol: channel.k8s.io" \
		--header "Sec-WebSocket-Key: MDEyMzQ1Njc4OWFiY2RlZg==" \
		"https://${CLUSTER_HOST_PORT}/api/v1/namespaces/${CURRENT_NAMESPACE}/pods/${pod_name}/exec?stdin=true&stdout=true&stderr=true&command=sh&command=-e&command=test.sh" \
		> "${artifacts_dir}/wss-output.log"

	# Check for exactly 1 occurrence of each error1-error2-error3 and out1-out2-out3
	for i in 1 2 3; do
		error_count=$(LC_ALL="C" grep --text -c "error${i}" "${artifacts_dir}/wss-output.log" || true)
		if [[ "${error_count}" -ne 1 ]]; then
			info "✗ Test FAILED: Expected exactly 1 occurrence of 'error${i}', found ${error_count}" >&2
			return 1
		fi

		out_count=$(LC_ALL="C" grep --text -c "out${i}" "${artifacts_dir}/wss-output.log" || true)
		if [[ "${out_count}" -ne 1 ]]; then
			info "✗ Test FAILED: Expected exactly 1 occurrence of 'out${i}', found ${out_count}" >&2
			return 1
		fi
	done
}


teardown() {
	# Show pod logs and status for debugging
	kubectl describe pod "${pod_name}" -n "${CURRENT_NAMESPACE}" || true
	kubectl logs "${pod_name}" -c "${container_name}" -n "${CURRENT_NAMESPACE}" || true
	echo "### wss-output.log ###"
	cat "${artifacts_dir}/wss-output.log" || true
	echo "####################"

	# Clean up
	kubectl delete pod "${pod_name}" -n "${CURRENT_NAMESPACE}" || true

	rm -rf "${artifacts_dir}"
    rm "${test_yaml_file}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"

	teardown_common "${node}" "${node_start_time:-}"
}
