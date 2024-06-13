#!/usr/bin/env bats
#
# Copyright (c) 2024 Edgeless Systems GmbH
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."
	( [ "${KATA_HYPERVISOR}" == "qemu-tdx" ] || [ "${KATA_HYPERVISOR}" == "qemu-sev" ] || [ "${KATA_HYPERVISOR}" == "qemu-snp" ] ) && skip "https://github.com/kata-containers/kata-containers/issues/9846"

	pod_name="policy-pod-pvc"
	pvc_name="policy-dev"

	get_pod_config_dir

	correct_pod_yaml="${pod_config_dir}/k8s-policy-pod-pvc.yaml"
	incorrect_pod_yaml="${pod_config_dir}/k8s-policy-pod-pvc-incorrect.yaml"
	pvc_yaml="${pod_config_dir}/k8s-policy-pvc.yaml"

	# Save some time by executing genpolicy a single time.
	if [ "${BATS_TEST_NUMBER}" == "1" ]; then
		# Add policy to the correct pod yaml file
		auto_generate_policy "${pod_config_dir}" "${correct_pod_yaml}"
	fi

    # Start each test case with a copy of the correct yaml files.
	cp "${correct_pod_yaml}" "${incorrect_pod_yaml}"
}

@test "Successful pod with auto-generated policy" {
	kubectl create -f "${correct_pod_yaml}"
	kubectl create -f "${pvc_yaml}"
	kubectl wait --for=condition=Ready "--timeout=${timeout}" pod "${pod_name}"
}

# Common function for several test cases from this bats script.
test_pod_policy_error() {
	kubectl create -f "${incorrect_pod_yaml}"
	kubectl create -f "${pvc_yaml}"
	wait_for_blocked_request "CreateContainerRequest" "${pod_name}"
}

@test "Policy failure: unexpected device mount" {
	# Changing the location of a mounted device after policy generation should fail the policy check.
	yq -i \
		'.spec.containers[0].volumeDevices.[0].devicePath = "/dev/unexpected"' \
		"${incorrect_pod_yaml}" \

	test_pod_policy_error
}

teardown() {
	auto_generate_policy_enabled || skip "Auto-generated policy tests are disabled."
	( [ "${KATA_HYPERVISOR}" == "qemu-tdx" ] || [ "${KATA_HYPERVISOR}" == "qemu-sev" ] || [ "${KATA_HYPERVISOR}" == "qemu-snp" ] ) && skip "https://github.com/kata-containers/kata-containers/issues/9846"

	# Debugging information. Don't print the "Message:" line because it contains a truncated policy log.
	kubectl describe pod "${pod_name}" | grep -v "Message:"

	# Clean-up
	kubectl delete -f "${correct_pod_yaml}"
	kubectl delete -f "${pvc_yaml}"
	rm -f "${incorrect_pod_yaml}"
}
