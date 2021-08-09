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

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	[ ! -f "$pod_logs_file" ] || rm -f "$pod_logs_file"
}
