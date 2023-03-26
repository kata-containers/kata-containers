#
# Copyright (c) 2021 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"

setup() {
	extract_kata_env

	# Ensure setting seccomp mode is allowed on guest
	sudo sed -i 's/disable_guest_seccomp=true/disable_guest_seccomp=false/' ${RUNTIME_CONFIG_PATH}

	pod_name="seccomp-container"
	get_pod_config_dir
}

@test "Support seccomp runtime/default profile" {
	expected_seccomp_mode="2"
	# Create pod
	kubectl create -f "${pod_config_dir}/pod-seccomp.yaml"

	# Wait it to complete
	cmd="kubectl get pods ${pod_name} | grep Completed"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	# Expect Seccomp on mode 2 (filter)
	seccomp_mode="$(kubectl logs ${pod_name} | sed 's/Seccomp:\s*\([0-9]\)/\1/')"
	[ "$seccomp_mode" -eq "$expected_seccomp_mode" ]
}

teardown() {
	# For debugging purpose
	echo "seccomp mode is ${seccomp_mode}, expected $expected_seccomp_mode"
	kubectl describe "pod/${pod_name}"

	kubectl delete -f "${pod_config_dir}/pod-seccomp.yaml" || true
	sudo sed -i 's/disable_guest_seccomp=false/disable_guest_seccomp=true/'\
		${RUNTIME_CONFIG_PATH}
}
