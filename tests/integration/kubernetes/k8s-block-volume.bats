#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	 case "${KATA_HYPERVISOR}" in
		qemu-runtime-rs|qemu-se-runtime-rs)
			skip "See: https://github.com/kata-containers/kata-containers/issues/10373" ;;
		fc|stratovirt)
			skip "See: https://github.com/kata-containers/kata-containers/issues/10873" ;;
	 esac

	get_pod_config_dir

	node="$(get_one_kata_node)"
	pod_name="pod-block-pv"
	volume_name="block-loop-pv"
	volume_claim="block-loop-pvc"
	ctr_dev_path="/dev/xda"
	vol_capacity="500M"

	# Create Loop Device
	tmp_disk_image=$(exec_host "$node" mktemp --tmpdir disk.XXXXXX.img)
	exec_host "$node" truncate "$tmp_disk_image" --size "$vol_capacity"
	loop_dev=$(exec_host "$node" sudo losetup -f)
	exec_host "$node" sudo losetup "$loop_dev" "$tmp_disk_image"
}

@test "Block Storage Support" {
	# Create Storage Class
	kubectl create -f volume/local-storage.yaml

	# Create Persistent Volume
	tmp_pv_yaml=$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)
	sed -e "s|LOOP_DEVICE|${loop_dev}|" volume/block-loop-pv.yaml > "$tmp_pv_yaml"
    node_name="$(kubectl get node -o name)"
	sed -i "s|HOSTNAME|${node_name##node/}|" "$tmp_pv_yaml"
	sed -i "s|CAPACITY|${vol_capacity}|" "$tmp_pv_yaml"
	kubectl create -f "$tmp_pv_yaml"
	cmd="kubectl get pv/${volume_name} | grep Available"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create Persistent Volume Claim
	tmp_pvc_yaml=$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)
	sed -e "s|CAPACITY|${vol_capacity}|" volume/block-loop-pvc.yaml > "$tmp_pvc_yaml"
	kubectl create -f "$tmp_pvc_yaml"

	# Create Workload using Volume
	tmp_pod_yaml=$(mktemp --tmpdir pod-pv.XXXXX.yaml)
	sed -e "s|DEVICE_PATH|${ctr_dev_path}|" "${pod_config_dir}/${pod_name}.yaml" > "$tmp_pod_yaml"

	ctr_mount_path="/mnt"
	ctr_message="Hello World"
	ctr_file="${ctr_mount_path}/file.txt"

	# Add policy to the workload.
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	mkfs_command="mkfs.ext4 ${ctr_dev_path}"
	mkfs_command_exec=(sh -c "${mkfs_command}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${mkfs_command_exec[@]}"

	mount_command="mount ${ctr_dev_path} ${ctr_mount_path}"
	mount_command_exec=(sh -c "${mount_command}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${mount_command_exec[@]}"

	echo_command="echo ${ctr_message} > ${ctr_file}"
	echo_command_exec=(sh -c "${echo_command}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${echo_command_exec[@]}"

	grep_command="grep '${ctr_message}' ${ctr_file}"
	grep_command_exec=(sh -c "${grep_command}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${grep_command_exec[@]}"

	auto_generate_policy "${policy_settings_dir}" "${tmp_pod_yaml}"

	# Start the workload.
	kubectl create -f "$tmp_pod_yaml"
	kubectl wait --for condition=ready --timeout=$timeout "pod/${pod_name}"

	# Verify persistent volume claim is bound
	kubectl get "pvc/${volume_claim}" | grep "Bound"

	# make fs, mount device and write on it
	kubectl exec "$pod_name" -- "${mkfs_command_exec[@]}"
	kubectl exec "$pod_name" -- "${mount_command_exec[@]}"
	kubectl exec "$pod_name" -- "${echo_command_exec[@]}"
	kubectl exec "$pod_name" -- "${grep_command_exec[@]}"
}

teardown() {
	case "${KATA_HYPERVISOR}" in
		qemu-runtime-rs|qemu-se-runtime-rs)
			skip "See: https://github.com/kata-containers/kata-containers/issues/10373" ;;
		fc|stratovirt)
			skip "See: https://github.com/kata-containers/kata-containers/issues/10873" ;;
	 esac
	 # Debugging information
	kubectl describe "pod/$pod_name"

	# Delete k8s resources
	kubectl delete pod "$pod_name"
	kubectl delete pvc "$volume_claim"
	kubectl delete pv "$volume_name"
	kubectl delete storageclass local-storage

	# Delete temporary yaml files
	rm -f "$tmp_pv_yaml"
	rm -f "$tmp_pvc_yaml"
	rm -f "$tmp_pod_yaml"

	# Remove image and loop device
	exec_host "$node" sudo losetup -d "$loop_dev"
	exec_host "$node" rm -f "$tmp_disk_image"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
