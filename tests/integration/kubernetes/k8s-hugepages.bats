#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"
issue="https://github.com/kata-containers/runtime/issues/2172"

setup() {
	skip "test not working see: ${issue}"
	export KUBECONFIG="$HOME/.kube/config"
	extract_kata_env

	# Enable hugepages
	sudo sed -i 's/#enable_hugepages = true/enable_hugepages = true/g' ${RUNTIME_CONFIG_PATH}

	pod_name="test-env"
	get_pod_config_dir
}

@test "Hugepages" {
	skip "test not working see: ${issue}"
	# Create pod
	kubectl create -f "${pod_config_dir}/pod-env.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Print environment variables
	cmd="printenv"
	kubectl exec $pod_name -- sh -c $cmd | grep "MY_POD_NAME=$pod_name"
}


@test "Hugepages and sandbox cgroup" {
	skip "test not working see: ${issue}"
	# Enable sandbox_cgroup_only
	sudo sed -i 's/sandbox_cgroup_only=false/sandbox_cgroup_only=true/g' ${RUNTIME_CONFIG_PATH}

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-env.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Print environment variables
	cmd="printenv"
	kubectl exec $pod_name -- sh -c $cmd | grep "MY_POD_NAME=$pod_name"

	# Disable sandbox_cgroup_only
	sudo sed -i 's/sandbox_cgroup_only=false/sandbox_cgroup_only=true/g' ${RUNTIME_CONFIG_PATH}
}

teardown() {
	skip "test not working see: ${issue}"
	kubectl delete pod "$pod_name"

	# Disable hugepages
	sudo sed -i 's/enable_hugepages = true/#enable_hugepages = true/g' ${RUNTIME_CONFIG_PATH}
}
