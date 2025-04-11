#!/usr/bin/env bats
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

setup() {
	ensure_yq

	pushd "${repo_root_dir}"

	# We expect 2 runtime classes because:
	# * `kata` is the default runtimeclass created, basically an alias for `kata-${KATA_HYPERVISOR}`.
	# * `kata-${KATA_HYPERVISOR}` is the other one
	#    * As part of the tests we're only deploying the specific runtimeclass that will be used, instead of all of them.
	expected_runtime_classes=2

	# We expect both runtime classes to have the same handler: kata-${KATA_HYPERVISOR}
	expected_handlers_re=( \
		"kata\s+kata-${KATA_HYPERVISOR}" \
		"kata-${KATA_HYPERVISOR}\s+kata-${KATA_HYPERVISOR}" \
	)


	# Set the latest image, the one generated as part of the PR, to be used as part of the tests
	export HELM_IMAGE_REFERENCE="${DOCKER_REGISTRY}/${DOCKER_REPO}"
	export HELM_IMAGE_TAG="${DOCKER_TAG}"

	# Enable debug for Kata Containers
	export HELM_DEBUG="true"

	# Create the runtime class only for the shim that's being tested
	export HELM_SHIMS="${KATA_HYPERVISOR}"

	# Set the tested hypervisor as the default `kata` shim
	export HELM_DEFAULT_SHIM="${KATA_HYPERVISOR}"

	# Let the `kata-deploy` create the default `kata` runtime class
	export HELM_CREATE_DEFAULT_RUNTIME_CLASS="true"

	HOST_OS=""
        if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
                HOST_OS="${KATA_HOST_OS}"
        fi
	export HELM_HOST_OS="${HOST_OS}"

	export HELM_K8S_DISTRIBUTION="${KUBERNETES}"

	helm_helper

	echo "::group::kata-deploy logs"
	kubectl -n kube-system logs --tail=100 -l name=kata-deploy
	echo "::endgroup::"

	echo "::group::Runtime classes"
	kubectl get runtimeclass
	echo "::endgroup::"

	popd
}

@test "Test runtimeclasses are being properly created and container runtime not broken" {
	# We filter `kata-mshv-vm-isolation` out as that's present on AKS clusters, but that's not coming from kata-deploy
	current_runtime_classes=$(kubectl get runtimeclasses | grep -v "kata-mshv-vm-isolation" | grep "kata" | wc -l)
	[[ ${current_runtime_classes} -eq ${expected_runtime_classes} ]]

	for handler_re in ${expected_handlers_re[@]}
	do
		kubectl get runtimeclass | grep -E "${handler_re}"
	done

	# Ensure that kata-deploy didn't corrupt containerd config, by trying to get the container runtime and node status
	echo "::group::kubectl node debug"
	kubectl get node -o wide
	kubectl describe nodes
	echo "::endgroup::"

	# Wait to see if the nodes get back into Ready state - if not then containerd might be having issues
	kubectl wait nodes --timeout=60s --all --for condition=Ready=True

	# Check that the container runtime verison doesn't have unknown, which happens when containerd can't start properly
	container_runtime_version=$(kubectl get nodes --no-headers -o custom-columns=CONTAINER_RUNTIME:.status.nodeInfo.containerRuntimeVersion)
	[[ ${container_runtime_version} != *"containerd://Unknown"* ]]
}

teardown() {
	pushd "${repo_root_dir}"

	helm uninstall --namespace=kube-system kata-deploy --wait
	kubectl -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod

	popd
}
