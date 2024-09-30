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
	[ "${KATA_HYPERVISOR}" = "qemu-se" ] && \
		skip "See: https://github.com/kata-containers/kata-containers/issues/10002"
	pod_name="sharevol-kata"
	get_pod_config_dir
	pod_logs_file=""

	yaml_file="${pod_config_dir}/pod-empty-dir.yaml"
	add_allow_all_policy_to_yaml "${yaml_file}"
}

@test "Empty dir volumes" {
	# Create the pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check volume mounts
	cmd="mount | grep cache"
	kubectl exec $pod_name -- sh -c "$cmd" | grep "/tmp/cache type tmpfs"

	# Check it can write up to the volume limit (50M)
	cmd="dd if=/dev/zero of=/tmp/cache/file1 bs=1M count=50; echo $?"
	kubectl exec $pod_name -- sh -c "$cmd" | tail -1 | grep 0
}

@test "Empty dir volume when FSGroup is specified with non-root container" {
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
		kubectl logs "$pod_name" "$container" > "$pod_logs_file"
		# Check owner UID of file
		uid=$(cat $pod_logs_file | grep 'owner UID of' | sed 's/.*:\s//')
		assert_equal "1001" "$uid"
		# Check owner GID of file
		gid=$(cat $pod_logs_file | grep 'owner GID of' | sed 's/.*:\s//')
		assert_equal "123" "$gid"
	done
}

teardown() {
	[ "${KATA_HYPERVISOR}" = "qemu-se" ] && \
		skip "See: https://github.com/kata-containers/kata-containers/issues/10002"
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	[ ! -f "$pod_logs_file" ] || rm -f "$pod_logs_file"
}
