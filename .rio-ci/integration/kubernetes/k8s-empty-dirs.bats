#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

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
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	pod_name="sharevol-kata"
	get_pod_config_dir
	pod_logs_file=""
	mem_size_pod_name="empty-dir-mem-kata"
}

@test "Empty dir volumes" {
	# Create the pod
	pcl "${pod_config_dir}/pod-empty-dir.pcl" | kubectl create -f -

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check volume mounts
	cmd="mount | grep cache"
	kubectl exec $pod_name -- sh -c "$cmd" | grep "/tmp/cache type virtiofs"
}

@test "Empty dir volume when FSGroup is specified with non-root container" {
	# This is a reproducer of k8s e2e "[sig-storage] EmptyDir volumes when FSGroup is specified [LinuxOnly] [NodeFeature:FSGroup] new files should be created with FSGroup ownership when container is non-root" test
	pcl	"${pod_config_dir}/pod-empty-dir-fsgroup.pcl" | kubectl create -f -
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

@test "Empty dir volume occupies sandbox memory" {
	kubectl create -f "${pod_config_dir}/pod-empty-dir-occupy-mem.yaml"
	cmd="kubectl get pods/$mem_size_pod_name -o jsonpath='{.status.phase}' | grep Running"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	echo "size=" $(kubectl logs "$mem_size_pod_name")

	# ensure that microvm empty volume occupies entire memory available to sandbox
	kubectl exec -it pods/$mem_size_pod_name -- sh -c "mount | grep test-volume | size=2621440k"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"
	kubectl describe "pod/$mem_size_pod_name"

	kubectl delete pod "$pod_name"
	kubectl delete pod "$mem_size_pod_name"
	
	[ ! -f "$pod_logs_file" ] || rm -f "$pod_logs_file"
}
