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
	setup_common
	get_pod_config_dir

    pod_name="hostpath-kmsg"
	yaml_file="${pod_config_dir}/pod-hostpath-kmsg.yaml"

	cmd_mountinfo=(sh -c "grep /dev/kmsg /proc/self/mountinfo")
	cmd_stat=(sh -c "stat -c '%t,%T' /dev/kmsg")
	cmd_head=(sh -c "head -10 /dev/kmsg")

    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_mountinfo[@]}"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_stat[@]}"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_head[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "/dev hostPath volume bind mounts the guest device and skips virtio-fs" {
	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	# Check the mount info.

	mount_info="$(kubectl exec "${pod_name}" -- "${cmd_mountinfo[@]}")"
	read root mountpoint fstype < <(awk '{print $4, $5, $9}' <<< "$mount_info")

	[ "$root" == "/kmsg" ] # Would look like "/<CONTAINER_ID>-<RANDOM_ID>-kmsg" with virtio-fs.
	[ "$mountpoint" == "/dev/kmsg" ]
	[ "$fstype" == "devtmpfs" ] # Would be "virtiofs" with virtio-fs.

	# Check the device major/minor.

	majminor="$(kubectl exec "${pod_name}" -- "${cmd_stat[@]}")"
	[ "$majminor" == "1,b" ]

	# Check that the device is actually accessible.

	kubectl exec "${pod_name}" -- "${cmd_head[@]}"
}

teardown() {
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
