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

	configmap_name="policy-configmap"
	pod_name="policy-pod"

	get_pod_config_dir

	correct_configmap_yaml="${pod_config_dir}/k8s-policy-configmap.yaml"
	pre_generate_configmap_yaml="${pod_config_dir}/k8s-policy-configmap-pre-generation.yaml"
	incorrect_configmap_yaml="${pod_config_dir}/k8s-policy-configmap-incorrect.yaml"
	testcase_pre_generate_configmap_yaml="${pod_config_dir}/k8s-policy-configmap-testcase-pre-generation.yaml"

	correct_pod_yaml="${pod_config_dir}/k8s-policy-pod.yaml"
	pre_generate_pod_yaml="${pod_config_dir}/k8s-policy-pod-pre-generation.yaml"
	incorrect_pod_yaml="${pod_config_dir}/k8s-policy-pod-incorrect.yaml"
	testcase_pre_generate_pod_yaml="${pod_config_dir}/k8s-policy-pod-testcase-pre-generation.yaml"


    # Save some time by executing genpolicy a single time.
    if [ "${BATS_TEST_NUMBER}" == "1" ]; then
		# Save pre-generated yaml files
		cp "${correct_configmap_yaml}" "${pre_generate_configmap_yaml}" 
		cp "${correct_pod_yaml}" "${pre_generate_pod_yaml}"

		# Add policy to the correct pod yaml file
		auto_generate_policy "${pod_config_dir}" "${correct_pod_yaml}" "${correct_configmap_yaml}"
	fi

    # Start each test case with a copy of the correct yaml files.
	cp "${correct_configmap_yaml}" "${incorrect_configmap_yaml}"
	cp "${correct_pod_yaml}" "${incorrect_pod_yaml}"

	# Also give each testcase a copy of the pre-generated yaml files.
	cp "${pre_generate_configmap_yaml}" "${testcase_pre_generate_configmap_yaml}"
	cp "${pre_generate_pod_yaml}" "${testcase_pre_generate_pod_yaml}"
}

@test "Successful pod with auto-generated policy" {
	kubectl create -f "${correct_configmap_yaml}"
	kubectl create -f "${correct_pod_yaml}"
	kubectl wait --for=condition=Ready "--timeout=${timeout}" pod "${pod_name}"
}

@test "Successful pod with auto-generated policy and runtimeClassName filter" {
	runtime_class_name=$(yq ".spec.runtimeClassName" < "${testcase_pre_generate_pod_yaml}")

	auto_generate_policy "${pod_config_dir}" "${testcase_pre_generate_pod_yaml}" "${testcase_pre_generate_configmap_yaml}" \
		"--runtime-class-names=other-runtime-class-name --runtime-class-names=${runtime_class_name}" 

	kubectl create -f "${testcase_pre_generate_configmap_yaml}"
	kubectl create -f "${testcase_pre_generate_pod_yaml}"
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
	yq -i \
		'.spec.containers[0].image = "quay.io/footloose/ubuntu18.04:latest"' \
		"${incorrect_pod_yaml}"

	test_pod_policy_error
}

@test "Policy failure: unexpected privileged security context" {
    # Changing the pod spec after generating its policy will cause CreateContainer to be denied.
	yq -i \
		'.spec.containers[0].securityContext.privileged = true' \
		"${incorrect_pod_yaml}"

	test_pod_policy_error
}

@test "Policy failure: unexpected terminationMessagePath" {
    # Changing the pod spec after generating its policy will cause CreateContainer to be denied.
	yq -i \
		'.spec.containers[0].terminationMessagePath = "/dev/termination-custom-log"' \
		"${incorrect_pod_yaml}"

	test_pod_policy_error
}

@test "Policy failure: unexpected hostPath volume mount" {
	# Changing the pod spec after generating its policy will cause CreateContainer to be denied.
  yq -i \
    '.spec.containers[0].volumeMounts += [{"name": "mountpoint-dir", "mountPath": "/var/lib/kubelet/pods"}]' \
    "${incorrect_pod_yaml}"

  yq -i \
    '.spec.volumes += [{"hostPath": {"path": "/var/lib/kubelet/pods", "type": "DirectoryOrCreate"}, "name": "mountpoint-dir"}]' \
    "${incorrect_pod_yaml}"

	test_pod_policy_error
}

@test "Policy failure: unexpected config map" {
	yq -i \
		'.data.data-2 = "foo"' \
		"${incorrect_configmap_yaml}"

	# These commands are different from the test_pod_policy_error() commands above
	# because in this case an incorrect config map spec is used.
	kubectl create -f "${incorrect_configmap_yaml}"
	kubectl create -f "${correct_pod_yaml}"
	wait_for_blocked_request "CreateContainerRequest" "${pod_name}"
}

@test "Policy failure: unexpected lifecycle.postStart.exec.command" {
	# Add a postStart command after generating the policy and verify that the post
	# start hook command gets blocked by policy.
	yq -i \
		'.spec.containers[0].lifecycle.postStart.exec.command += ["echo"]' \
		"${incorrect_pod_yaml}"

	yq -i \
		'.spec.containers[0].lifecycle.postStart.exec.command += ["hello"]' \
		"${incorrect_pod_yaml}"

	kubectl create -f "${correct_configmap_yaml}"
	kubectl create -f "${incorrect_pod_yaml}"

	command="kubectl describe pod ${pod_name} | grep FailedPostStartHook"
	info "Waiting ${wait_time} seconds for: ${command}"

	# Don't print the "Message:" line because it contains a truncated policy log.
	waitForProcess "${wait_time}" "$sleep_time" "${command}" | grep -v "Message:"
}

@test "RuntimeClassName filter: no policy" {
	# Solve a bats warning:
	# BW02: Using flags on `run` requires at least BATS_VERSION=1.5.0.
	# Use `bats_require_minimum_version 1.5.0` to fix this message.
	bats_require_minimum_version 1.5.0

	# The policy should not be generated because the pod spec does not have a runtimeClassName.
	runtime_class_name=$(yq ".spec.runtimeClassName" < "${testcase_pre_generate_pod_yaml}")

	auto_generate_policy "${pod_config_dir}" "${testcase_pre_generate_pod_yaml}" "${testcase_pre_generate_configmap_yaml}" \
		"--runtime-class-names=other-${runtime_class_name}"

	# Check that the pod yaml does not contain a policy annotation.
	run ! grep -q "io.katacontainers.config.agent.policy" "${testcase_pre_generate_pod_yaml}"
}

teardown() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

	# Debugging information. Don't print the "Message:" line because it contains a truncated policy log.
	kubectl describe pod "${pod_name}" | grep -v "Message:"

	# Clean-up
	kubectl delete pod "${pod_name}"
	kubectl delete configmap "${configmap_name}"
	rm -f "${incorrect_pod_yaml}"
	rm -f "${incorrect_configmap_yaml}"
	rm -f "${testcase_pre_generate_pod_yaml}"
	rm -f "${testcase_pre_generate_configmap_yaml}"
}
