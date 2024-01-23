#!/usr/bin/env bats
#
# Copyright (c) 2020 Ant Financial
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	pod_name="busybox"
	first_container_name="first-test-container"
	second_container_name="second-test-container"
	yaml_file="${pod_config_dir}/busybox-pod.yaml"
}

@test "Kubectl exec" {
	# TODO: disabled due to 8868
	# auto_generate_policy "${yaml_file}"

	# Create the pod
	kubectl create -f "${yaml_file}"

	# Get pod specification
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Run commands in Pod
	## Cases for -it options
	# TODO: enable -i option after updated to new CRI-O
	# see: https://github.com/kata-containers/tests/issues/2770
	# kubectl exec -i "$pod_name" -- ls -tl /
	# kubectl exec -it "$pod_name" -- ls -tl /
	kubectl exec "$pod_name" -- date

	## Case for stdin
	kubectl exec -i "$pod_name" -- sh <<-EOF
echo abc > /tmp/abc.txt
grep abc /tmp/abc.txt
exit
EOF

	## Case for return value
	### Command return non-zero code
	run bash -c "kubectl exec -i $pod_name -- sh <<-EOF
exit 123
EOF"
	echo "run status: $status" 1>&2
	echo "run output: $output" 1>&2
	[ "$status" -eq 123 ]

	## Cases for target container
	### First container
	container_name=$(kubectl exec $pod_name -c $first_container_name -- env | grep CONTAINER_NAME)
	[ "$container_name" == "CONTAINER_NAME=$first_container_name" ]

	### Second container
	container_name=$(kubectl exec $pod_name -c $second_container_name -- env | grep CONTAINER_NAME)
	[ "$container_name" == "CONTAINER_NAME=$second_container_name" ]

}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
