#!/usr/bin/env bats
#
# Copyright (c) 2025 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common || die "setup_common failed"

	pod_name="privileged"
	yaml_file="${pod_config_dir}/pod-privileged.yaml"
	nesting_pod_name="cgroup-v2-nesting"
	nesting_yaml_file="${pod_config_dir}/pod-cgroup-v2-nesting.yaml"

	cmd_nsenter=(nsenter --mount=/proc/1/ns/mnt true)

	# /sys/fs/cgroup in the container is rooted at the container cgroup, so the
	# leaf PID 1 moved into can be addressed by name. The exec process cannot
	# check /proc/self/cgroup instead: it gets a cgroup namespace rooted at
	# whichever cgroup it joined, so that always reads "/".
	cmd_dind_cgroup=(sh -c 'cat /sys/fs/cgroup/init/cgroup.procs; grep -qx "$$" /sys/fs/cgroup/init/cgroup.procs')
	cmd_systemd_cgroup=(sh -c 'cat /sys/fs/cgroup/init.scope/cgroup.procs; grep -qx "$$" /sys/fs/cgroup/init.scope/cgroup.procs')

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_nsenter[@]}"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_dind_cgroup[@]}"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_systemd_cgroup[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
	auto_generate_policy "${policy_settings_dir}" "${nesting_yaml_file}"
}

# This should succeed because the CI uses kata-deploy which sets
# privileged_without_host_devices to true.
@test "Privileged pod runs and is able to execute privileged operations" {
	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"
	kubectl exec "${pod_name}" -- "${cmd_nsenter[@]}"
}

@test "Exec into nested cgroup v2 containers" {
	kubectl apply -f "${nesting_yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${nesting_pod_name}"

	for container in dind systemd; do
		waitForProcess "${wait_time}" "${sleep_time}" \
			"kubectl logs ${nesting_pod_name} -c ${container} | grep -x READY"
	done

	# The exec process must join PID 1 in the leaves used by DinD and systemd.
	# Attaching it to the container cgroup instead fails with EBUSY, because
	# that cgroup became an inner node once the leaf enabled a controller.
	kubectl exec "${nesting_pod_name}" -c dind -- "${cmd_dind_cgroup[@]}"
	kubectl exec "${nesting_pod_name}" -c systemd -- "${cmd_systemd_cgroup[@]}"
}

teardown() {
	echo "Pod logs:"
	kubectl logs "${pod_name}" || true
	kubectl logs "${nesting_pod_name}" --all-containers || true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
