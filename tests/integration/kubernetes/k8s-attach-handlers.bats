#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	versions_file="${BATS_TEST_DIRNAME}/../../versions.yaml"
	nginx_version=$("${GOPATH}/bin/yq" read "$versions_file" "docker_images.nginx.version")
	nginx_image="nginx:$nginx_version"

	export KUBECONFIG="$HOME/.kube/config"
	pod_name="handlers"

	get_pod_config_dir
}

@test "Running with postStart and preStop handlers" {
	# Create yaml
	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/lifecycle-events.yaml" > "${pod_config_dir}/test-lifecycle-events.yaml"

	# Create the pod with postStart and preStop handlers
	kubectl create -f "${pod_config_dir}/test-lifecycle-events.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Check postStart message
	display_message="cat /usr/share/message"
	check_postStart=$(kubectl exec $pod_name -- sh -c "$display_message" | grep "Hello from the postStart handler")
}

teardown(){
	rm -f "${pod_config_dir}/test-lifecycle-events.yaml"
	kubectl delete pod "$pod_name"
}
