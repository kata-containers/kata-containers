#!/usr/bin/env bats
#
# Copyright (c) 2024 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	policy_tests_enabled || skip "Policy tests are disabled."

	configmap_name="policy-configmap"
	pod_name="policy-pod"

	get_pod_config_dir

	correct_configmap_yaml="${pod_config_dir}/k8s-policy-configmap.yaml"
	incorrect_configmap_yaml="${pod_config_dir}/k8s-policy-configmap-incorrect.yaml"

	correct_pod_yaml="${pod_config_dir}/k8s-policy-pod.yaml"
	incorrect_pod_yaml="${pod_config_dir}/k8s-policy-pod-incorrect.yaml"

    # Save some time by executing genpolicy a single time.
    if [ "${BATS_TEST_NUMBER}" == "1" ]; then
		# Add policy to the correct pod yaml file
		auto_generate_policy "${pod_config_dir}" "${correct_pod_yaml}" "${correct_configmap_yaml}"
	fi

    # Start each test case with a copy of the correct yaml files.
	cp "${correct_configmap_yaml}" "${incorrect_configmap_yaml}"
	cp "${correct_pod_yaml}" "${incorrect_pod_yaml}"
}

@test "Successful pod with auto-generated policy" {
	kubectl create -f "${correct_configmap_yaml}"
	kubectl create -f "${correct_pod_yaml}"
	kubectl wait --for=condition=Ready "--timeout=${timeout}" pod "${pod_name}"
}

# Common function for several test cases from this bats script.
test_pod_policy_error() {
	kubectl create -f "${correct_configmap_yaml}"
	kubectl create -f "${incorrect_pod_yaml}"
	wait_for_blocked_request "CreateContainerRequest" "${pod_name}"
}

@test "Policy failure: unexpected container image" {
	# Change the container image after generating the policy. The different image has
	# different attributes (e.g., different command line) so the policy will reject it.
	yq write -i \
		"${incorrect_pod_yaml}" \
		"spec.containers[0].image" \
		"quay.io/footloose/ubuntu18.04:latest"

	test_pod_policy_error
}

@test "Policy failure: unexpected privileged security context" {
    # Changing the pod spec after generating its policy will cause CreateContainer to be denied.
	yq write -i \
		"${incorrect_pod_yaml}" \
		'spec.containers[0].securityContext.privileged' \
		"true"

	test_pod_policy_error
}

@test "Policy failure: unexpected terminationMessagePath" {
    # Changing the pod spec after generating its policy will cause CreateContainer to be denied.
	yq write -i \
		"${incorrect_pod_yaml}" \
		'spec.containers[0].terminationMessagePath' \
		"/dev/termination-custom-log"

	test_pod_policy_error
}

@test "Policy failure: unexpected hostPath volume mount" {
	# Changing the pod spec after generating its policy will cause CreateContainer to be denied.
	yq write -i \
		"${incorrect_pod_yaml}" \
		"spec.containers[0].volumeMounts.[+].name" \
		"mountpoint-dir"

	yq write -i \
		"${incorrect_pod_yaml}" \
		"spec.containers[0].volumeMounts.[-1].mountPath" \
		"/var/lib/kubelet/pods"

	yq write -i \
		"${incorrect_pod_yaml}" \
		"spec.volumes.[+].hostPath.path" \
		"/var/lib/kubelet/pods"

	yq write -i \
		"${incorrect_pod_yaml}" \
		"spec.volumes.[-1].hostPath.type" \
		"DirectoryOrCreate"

	yq write -i \
		"${incorrect_pod_yaml}" \
		"spec.volumes.[-1].name" \
		"mountpoint-dir"

	test_pod_policy_error
}

@test "Policy failure: unexpected config map" {
	yq write -i \
		"${incorrect_configmap_yaml}" \
		'data.data-2' \
		"foo"

	# These commands are different from the test_pod_policy_error() commands above
	# because in this case an incorrect config map spec is used.
	kubectl create -f "${incorrect_configmap_yaml}"
	kubectl create -f "${correct_pod_yaml}"
	wait_for_blocked_request "CreateContainerRequest" "${pod_name}"
}

@test "Policy failure: unexpected lifecycle.postStart.exec.command" {
	# Add a postStart command after generating the policy and verify that the post
	# start hook command gets blocked by policy.
	yq write -i \
		"${incorrect_pod_yaml}" \
		'spec.containers[0].lifecycle.postStart.exec.command.[+]' \
		"echo"

	yq write -i \
		"${incorrect_pod_yaml}" \
		'spec.containers[0].lifecycle.postStart.exec.command.[+]' \
		"hello"

	kubectl create -f "${correct_configmap_yaml}"
	kubectl create -f "${incorrect_pod_yaml}"

	command="kubectl describe pod ${pod_name} | grep FailedPostStartHook"
	info "Waiting ${wait_time} seconds for: ${command}"

	# Don't print the "Message:" line because it contains a truncated policy log.
	waitForProcess "${wait_time}" "$sleep_time" "${command}" | grep -v "Message:"
}

teardown() {
	policy_tests_enabled || skip "Policy tests are disabled."

	# Debugging information. Don't print the "Message:" line because it contains a truncated policy log.
	kubectl describe pod "${pod_name}" | grep -v "Message:"

	# Clean-up
	kubectl delete pod "${pod_name}"
	kubectl delete configmap "${configmap_name}"
	rm -f "${incorrect_pod_yaml}"
	rm -f "${incorrect_configmap_yaml}"
}
