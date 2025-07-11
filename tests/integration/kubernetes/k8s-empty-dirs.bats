#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

assert_equal() {
	local expected=$1
	local actual=$2
	if [[ "$expected" != "$actual" ]]; then
	echo "expected: $expected, got: $actual"
	return 1
	fi
}

setup() {
	pod_name="sharevol-kata"
	get_pod_config_dir
	pod_logs_file=""

	yaml_file="${pod_config_dir}/pod-empty-dir.yaml"

	# Add policy to yaml
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	mount_command=(sh -c "mount | grep cache")
	add_exec_to_policy_settings "${policy_settings_dir}" "${mount_command[@]}"

	dd_command=(sh -c "dd if=/dev/zero of=/tmp/cache/file1 bs=1M count=50; echo $?")
	add_exec_to_policy_settings "${policy_settings_dir}" "${dd_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Empty dir volumes" {
	# Create the pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check volume mounts
	kubectl exec $pod_name -- "${mount_command[@]}" | grep "/tmp/cache type tmpfs"

	# Check it can write up to the volume limit (50M)
	kubectl exec $pod_name -- "${dd_command[@]}" | tail -1 | grep 0
}

@test "Empty dir volume when FSGroup is specified with non-root container" {
	local agnhost_name
	local agnhost_version
	local gid
	local image
	local logs
	local pod_file
	local pod_logs_file
	local uid

	[[ "${KATA_HYPERVISOR}" = qemu-se* ]] && \
		skip "See: https://github.com/kata-containers/kata-containers/issues/10002"
	# This is a reproducer of k8s e2e "[sig-storage] EmptyDir volumes when FSGroup is specified [LinuxOnly] [NodeFeature:FSGroup] new files should be created with FSGroup ownership when container is non-root" test
	pod_file="${pod_config_dir}/pod-empty-dir-fsgroup.yaml"
	agnhost_name="${container_images_agnhost_name}"
	agnhost_version="${container_images_agnhost_version}"
	image="${agnhost_name}:${agnhost_version}"

	# Try to avoid timeout by prefetching the image.
	sed -e "s#\${agnhost_image}#${image}#" "$pod_file" |\
		kubectl create -f -
	cmd="kubectl get pods ${pod_name} | grep Completed"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	pod_logs_file="$(mktemp)"
	for container in mounttest-container mounttest-container-2; do
		bats_unbuffered_info "Getting logs for $container"
		kubectl logs "$pod_name" "$container" > "$pod_logs_file"
		logs=$(cat $pod_logs_file)
		bats_unbuffered_info "Logs: $logs"

		# Check owner UID of file
		uid=$(cat $pod_logs_file | grep 'owner UID of' | sed 's/.*:\s//')
		assert_equal "1001" "$uid"
		# Check owner GID of file
		gid=$(cat $pod_logs_file | grep 'owner GID of' | sed 's/.*:\s//')
		assert_equal "123" "$gid"
	done
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	[ ! -f "$pod_logs_file" ] || rm -f "$pod_logs_file"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
