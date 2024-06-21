#
# Copyright (c) 2021 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR:-}" = "qemu-coco-dev" ] && \
		skip "This test fails intermittently for ${KATA_HYPERVISOR:-}"
	pod_name="seccomp-container"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/pod-seccomp.yaml"
	add_allow_all_policy_to_yaml "${yaml_file}"
}

@test "Support seccomp runtime/default profile" {
	expected_seccomp_mode="2"
	# Create pod
	kubectl create -f "${yaml_file}"

	# Wait it to complete
	cmd="kubectl get pods ${pod_name} | grep Completed"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	# Expect Seccomp on mode 2 (filter)
	seccomp_mode="$(kubectl logs ${pod_name} | sed 's/Seccomp:\s*\([0-9]\)/\1/')"
	[ "$seccomp_mode" -eq "$expected_seccomp_mode" ]
}

teardown() {
	[ "${KATA_HYPERVISOR:-}" = "qemu-coco-dev" ] && \
		skip "This test fails intermittently for ${KATA_HYPERVISOR:-}"
	# For debugging purpose
	echo "seccomp mode is ${seccomp_mode}, expected $expected_seccomp_mode"
	kubectl describe "pod/${pod_name}"

	kubectl delete -f "${yaml_file}" || true
}
