#!/usr/bin/env bats
#
# Copyright (c) 2024 Microsoft.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

issue="https://github.com/kata-containers/kata-containers/issues/10297"

setup() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

	configmap_name="policy-configmap"
	pod_name="policy-pod"
	priority_class_name="test-high-priority"

	get_pod_config_dir
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	
	exec_command=(printenv data-3)
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"

	correct_configmap_yaml="${pod_config_dir}/k8s-policy-configmap.yaml"
	pre_generate_configmap_yaml="${pod_config_dir}/k8s-policy-configmap-pre-generation.yaml"
	incorrect_configmap_yaml="${pod_config_dir}/k8s-policy-configmap-incorrect.yaml"
	testcase_pre_generate_configmap_yaml="${pod_config_dir}/k8s-policy-configmap-testcase-pre-generation.yaml"

	correct_pod_yaml="${pod_config_dir}/k8s-policy-pod.yaml"
	priority_class_yaml="${pod_config_dir}/k8s-priority-class.yaml"
	pre_generate_pod_yaml="${pod_config_dir}/k8s-policy-pod-pre-generation.yaml"
	incorrect_pod_yaml="${pod_config_dir}/k8s-policy-pod-incorrect.yaml"
	testcase_pre_generate_pod_yaml="${pod_config_dir}/k8s-policy-pod-testcase-pre-generation.yaml"

	kubectl create -f "${priority_class_yaml}"

    # Save some time by executing genpolicy a single time.
    if [ "${BATS_TEST_NUMBER}" == "1" ]; then
		# Work around #10297 if needed.
		prometheus_image_supported || replace_prometheus_image

		# Save pre-generated yaml files
		cp "${correct_configmap_yaml}" "${pre_generate_configmap_yaml}" 
		cp "${correct_pod_yaml}" "${pre_generate_pod_yaml}"

		# Add policy to the correct pod yaml file
		auto_generate_policy "${policy_settings_dir}" "${correct_pod_yaml}" "${correct_configmap_yaml}"
	fi

    # Start each test case with a copy of the correct yaml files.
	cp "${correct_configmap_yaml}" "${incorrect_configmap_yaml}"
	cp "${correct_pod_yaml}" "${incorrect_pod_yaml}"

	# Also give each testcase a copy of the pre-generated yaml files.
	cp "${pre_generate_configmap_yaml}" "${testcase_pre_generate_configmap_yaml}"
	cp "${pre_generate_pod_yaml}" "${testcase_pre_generate_pod_yaml}"
}

prometheus_image_supported() {
	[[ "${SNAPSHOTTER:-}" == "nydus" ]] && return 1
	return 0
}

replace_prometheus_image() {
	info "Replacing prometheus image with busybox to work around ${issue}"

	yq -i \
		'.spec.containers[0].name = "busybox"' \
		"${correct_pod_yaml}"
	yq -i \
		'.spec.containers[0].image = "quay.io/prometheus/busybox:latest"' \
		"${correct_pod_yaml}"
}

# Common function for several test cases from this bats script.
wait_for_pod_ready() {
	kubectl create -f "${correct_configmap_yaml}"
	kubectl create -f "${correct_pod_yaml}"
	kubectl wait --for=condition=Ready "--timeout=${timeout}" pod "${pod_name}"
}

@test "Successful pod with auto-generated policy" {
	wait_for_pod_ready
}

@test "Able to read env variables sourced from configmap using envFrom" {
	wait_for_pod_ready
	expected_env_var=$(kubectl exec "${pod_name}" -- "${exec_command[@]}")
	[ "$expected_env_var" = "value-3" ] || fail "expected_env_var is not equal to value-3"
}

@test "Successful pod with auto-generated policy and runtimeClassName filter" {
	runtime_class_name=$(yq ".spec.runtimeClassName" < "${testcase_pre_generate_pod_yaml}")

	auto_generate_policy "${pod_config_dir}" "${testcase_pre_generate_pod_yaml}" "${testcase_pre_generate_configmap_yaml}" \
		"--runtime-class-names=other-runtime-class-name --runtime-class-names=${runtime_class_name}" 

	kubectl create -f "${testcase_pre_generate_configmap_yaml}"
	kubectl create -f "${testcase_pre_generate_pod_yaml}"
	kubectl wait --for=condition=Ready "--timeout=${timeout}" pod "${pod_name}"
}

@test "Successful pod with auto-generated policy and custom layers cache path" {
	tmp_path=$(mktemp -d)

	auto_generate_policy "${pod_config_dir}" "${testcase_pre_generate_pod_yaml}" "${testcase_pre_generate_configmap_yaml}" \
		"--layers-cache-file-path=${tmp_path}/cache.json"

	[ -f "${tmp_path}/cache.json" ]
	rm -r "${tmp_path}"

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

@test "ExecProcessRequest tests" {
	wait_for_pod_ready

	# Execute commands allowed by the policy.
	pod_exec_allowed_command "${pod_name}" "echo" "livenessProbe" "test"
	pod_exec_allowed_command "${pod_name}" "echo" "-n" "readinessProbe with space characters"
	pod_exec_allowed_command "${pod_name}" "echo" "startupProbe" "test"

	# Try to execute commands disallowed by the policy.
	pod_exec_blocked_command "${pod_name}" "echo" "livenessProbe test"
	pod_exec_blocked_command "${pod_name}" "echo" "livenessProbe" "test2"
	pod_exec_blocked_command "${pod_name}" "echo" "livenessProbe" "test" "yes"
	pod_exec_blocked_command "${pod_name}" "echo" "livenessProbe" "test foo"
	pod_exec_blocked_command "${pod_name}" "echo" "hello"
}

@test "Successful pod: runAsUser having the same value as the UID from the container image" {
	prometheus_image_supported || skip "Test case not supported due to ${issue}"

	# This container image specifies user = "nobody" that corresponds to UID = 65534. Setting
	# the same value for runAsUser in the YAML file doesn't change the auto-generated Policy.
	yq -i \
		'.spec.containers[0].securityContext.runAsUser = 65534' \
		"${incorrect_pod_yaml}"

	kubectl create -f "${correct_configmap_yaml}"
	kubectl create -f "${incorrect_pod_yaml}"
	kubectl wait --for=condition=Ready "--timeout=${timeout}" pod "${pod_name}"
}

@test "Policy failure: unexpected UID = 0" {
	prometheus_image_supported || skip "Test case not supported due to ${issue}"

	# Change the container UID to 0 after the policy has been generated, and verify that the
	# change gets rejected by the policy. UID = 0 is the default value from genpolicy, but
	# this container image specifies user = "nobody" that corresponds to UID = 65534.
	yq -i \
		'.spec.containers[0].securityContext.runAsUser = 0' \
		"${incorrect_pod_yaml}"

	test_pod_policy_error
}

@test "Policy failure: unexpected UID = 1234" {
	# Change the container UID to 1234 after the policy has been generated, and verify that the
	# change gets rejected by the policy. This container image specifies user = "nobody" that
	# corresponds to UID = 65534.
	yq -i \
		'.spec.containers[0].securityContext.runAsUser = 1234' \
		"${incorrect_pod_yaml}"

	test_pod_policy_error
}

teardown() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."

	# Debugging information. Don't print the "Message:" line because it contains a truncated policy log.
	kubectl describe pod "${pod_name}" | grep -v "Message:"

	# Clean-up
	kubectl delete pod "${pod_name}"
	kubectl delete configmap "${configmap_name}"
	kubectl delete priorityClass "${priority_class_name}"
	rm -f "${incorrect_pod_yaml}"
	rm -f "${incorrect_configmap_yaml}"
	rm -f "${testcase_pre_generate_pod_yaml}"
	rm -f "${testcase_pre_generate_configmap_yaml}"
}
