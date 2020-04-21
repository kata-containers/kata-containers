#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	get_pod_config_dir

	pod_name="pod-block-pv"
	ctr_dev_path="/dev/xda"
	vol_capacity="500M"

	# Create Loop Device
	tmp_disk_image=$(mktemp --tmpdir disk.XXXXXX.img)
	sudo truncate "$tmp_disk_image" --size "$vol_capacity"
	loop_dev=$(sudo losetup -f)
	sudo losetup "$loop_dev" "$tmp_disk_image"
}

@test "Block Storage Support" {
	# Create Storage Class
	kubectl create -f volume/local-storage.yaml

	# Create Persistent Volume
	tmp_pv_yaml=$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)
	sed -e "s|LOOP_DEVICE|${loop_dev}|" volume/block-loop-pv.yaml > "$tmp_pv_yaml"
	sed -i "s|HOSTNAME|$(hostname)|" "$tmp_pv_yaml"
	sed -i "s|CAPACITY|${vol_capacity}|" "$tmp_pv_yaml"
	kubectl create -f "$tmp_pv_yaml"

	# Create Persistent Volume Claim
	tmp_pvc_yaml=$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)
	sed -e "s|CAPACITY|${vol_capacity}|" volume/block-loop-pvc.yaml > "$tmp_pvc_yaml"
	kubectl create -f "$tmp_pvc_yaml"

	# Create Workload using Volume
	tmp_pod_yaml=$(mktemp --tmpdir pod-pv.XXXXX.yaml)
	sed -e "s|DEVICE_PATH|${ctr_dev_path}|" "${pod_config_dir}/${pod_name}.yaml" > "$tmp_pod_yaml"
	kubectl create -f "$tmp_pod_yaml"
	kubectl wait --for condition=ready "pod/${pod_name}"

	# Verify persistent volume claim is bound
	kubectl get pvc | grep "Bound"

	# Add the e2fsprogs package to get mkfs.ext4 installed
	kubectl exec "$pod_name" -- sh -c "apk add --no-cache e2fsprogs"
	# make fs, mount device and write on it
	kubectl exec "$pod_name" -- sh -c "mkfs.ext4 $ctr_dev_path"
	ctr_mount_path="/mnt"
	ctr_message="Hello World"
	ctr_file="${ctr_mount_path}/file.txt"
	kubectl exec "$pod_name" -- sh -c "mount $ctr_dev_path $ctr_mount_path"
	kubectl exec "$pod_name" -- sh -c "echo $ctr_message > $ctr_file"
	kubectl exec "$pod_name" -- sh -c "grep '$ctr_message' $ctr_file"
}

teardown() {
	# Delete k8s resources
	kubectl delete pod "$pod_name"
	kubectl delete pvc block-loop-pvc
	kubectl delete pv block-loop-pv
	kubectl delete storageclass local-storage

	# Delete temporary yaml files
	rm -f "$tmp_pv_yaml"
	rm -f "$tmp_pvc_yaml"
	rm -f "$tmp_pod_yaml"

	# Remove image and loop device
	sudo losetup -d "$loop_dev"
	rm -f "$tmp_disk_image"
}

