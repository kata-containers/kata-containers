#!/usr/bin/env bats
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

IMAGE_REPO="ghcr.io/confidential-containers/test-container"

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir

	SSH_KEY_FILE="${pod_config_dir}/confidential/unencrypted/ssh/unencrypted"
}

@test "Test unencrypted confidential container launch success and verify that we are running in a secure enclave." {
	# Adjust the image tag that will be used
	sed -i -e "s|IMAGE_TAG|${DOCKER_TAG%%-*}|" "${pod_config}/pod-confidential-unencrypted.yaml"

	# Start the service/deployment/pod
	kubectl apply -f "${pod_config_dir}/pod-confidential-unencrypted.yaml"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	pod_name=$(kubectl get pod -o wide | grep "confidential-unencrypted" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=20s pod "${pod_name}"

	pod_ip=$(kubectl get service "confidential-unencrypted" -o jsonpath="{.spec.clusterIP}")

	# Look for SEV enabled in container dmesg output
	coco_enabled=$(ssh -i ${SSH_KEY_FILE} -o "StrictHostKeyChecking no" -o "PasswordAuthentication=no" root@${pod_ip} /bin/sh -c "$(get_remote_command_per_hypervisor)" || true)

	if [ -z "$coco_enabled" ]; then
		>&2 echo -e "Confidential compute is expected but not enabled."
		return 1
	fi
}

teardown() {
	# Debugging information
	kubectl describe "pod/${pod_name}" || true
	kubectl delete -f "${pod_config_dir}/pod-confidential-unencrypted.yaml" || true
}
