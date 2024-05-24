#!/usr/bin/env bats
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"

setup() {
	repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
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
	sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"

	# Enable debug for Kata Containers
	yq -i \
	  '.spec.template.spec.containers[0].env[1].value = "true"' \
	  "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	# Create the runtime class only for the shim that's being tested
	yq -i \
	  ".spec.template.spec.containers[0].env[2].value = \"${KATA_HYPERVISOR}\"" \
	  "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	# Set the tested hypervisor as the default `kata` shim
	yq -i \
	  ".spec.template.spec.containers[0].env[3].value = \"${KATA_HYPERVISOR}\"" \
	  "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	# Let the `kata-deploy` script take care of the runtime class creation / removal
	yq -i \
	  '.spec.template.spec.containers[0].env[4].value = "true"' \
	  "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	# Let the `kata-deploy` create the default `kata` runtime class
	yq -i \
	  '.spec.template.spec.containers[0].env[5].value = "true"' \
	  "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"

	if [ "${KATA_HOST_OS}" = "cbl-mariner" ]; then
		yq -i \
		  ".spec.template.spec.containers[0].env += [{\"name\": \"HOST_OS\", \"value\": \"${KATA_HOST_OS}\"}]" \
		  "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	fi

	echo "::group::Final kata-deploy.yaml that is used in the test"
	cat "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml" || die "Failed to setup the tests image"
	echo "::endgroup::"

	echo "::group::Debug overlays directory content"
	echo "Current working directory: $(pwd)"
	ls -la tools/packaging/kata-deploy/kata-deploy/overlays/
	echo "::endgroup::"

	kubectl apply -f "tools/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"
	if [ "${KUBERNETES}" = "k0s" ]; then
		kubectl apply -k "tools/packaging/kata-deploy/kata-deploy/overlays/k0s"
	elif [ "${KUBERNETES}" = "k3s" ]; then
		kubectl apply -k "tools/packaging/kata-deploy/kata-deploy/overlays/k3s"
	elif [ "${KUBERNETES}" = "rke2" ]; then
		kubectl apply -k "tools/packaging/kata-deploy/kata-deploy/overlays/rke2"
	else
		kubectl apply -f "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"
	fi

	local cmd="kubectl -n kube-system get -l name=kata-deploy pod 2>/dev/null | grep '\<Running\>'"

	if ! waitForProcess 240 10 "$cmd"; then
		echo "Kata-deploy pod is not running. Printing pod details for debugging:"
		kubectl -n kube-system get pods -o wide
		kubectl -n kube-system get pods -l name=kata-deploy -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}' | while read -r pod; do
			echo "Describing pod: $pod"
			kubectl -n kube-system describe pod "$pod"
		done

		echo "ERROR: kata-deploy pod is not running, tests will not be execute."
		echo "ERROR: setup() aborting tests..."
		return 1
	fi

	# Give some time for the pod to finish what's doing and have the
	# runtimeclasses properly created
	sleep 30s

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

	if [ "${KUBERNETES}" = "k0s" ]; then
		deploy_spec="-k \"tools/packaging/kata-deploy/kata-deploy/overlays/k0s\""
		cleanup_spec="-k \"tools/packaging/kata-deploy/kata-cleanup/overlays/k0s\""
	elif [ "${KUBERNETES}" = "k3s" ]; then
		deploy_spec="-k \"tools/packaging/kata-deploy/kata-deploy/overlays/k3s\""
		cleanup_spec="-k \"tools/packaging/kata-deploy/kata-cleanup/overlays/k3s\""
	elif [ "${KUBERNETES}" = "rke2" ]; then
		deploy_spec="-k \"tools/packaging/kata-deploy/kata-deploy/overlays/rke2\""
		cleanup_spec="-k \"tools/packaging/kata-deploy/kata-cleanup/overlays/rke2\""
	else
		deploy_spec="-f \"tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml\""
		cleanup_spec="-f \"tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml\""
	fi

	kubectl delete ${deploy_spec}
	kubectl -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod

	# Let the `kata-deploy` script take care of the runtime class creation / removal
	yq -i \
	  '.spec.template.spec.containers[0].env[4].value = "true"' \
	  "tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
	# Create the runtime class only for the shim that's being tested
	yq -i \
	  ".spec.template.spec.containers[0].env[2].value = \"${KATA_HYPERVISOR}\"" \
	  "tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
	# Set the tested hypervisor as the default `kata` shim
	yq -i \
	  ".spec.template.spec.containers[0].env[3].value = \"${KATA_HYPERVISOR}\"" \
	  "tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
	# Let the `kata-deploy` create the default `kata` runtime class
	yq -i \
	  '.spec.template.spec.containers[0].env[5].value = "true"' \
	  "tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml"

	sed -i -e "s|quay.io/kata-containers/kata-deploy:latest|${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}|g" "tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
	cat "tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml"
	grep "${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" "tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml" || die "Failed to setup the tests image"

	kubectl apply ${cleanup_spec}
	sleep 30s

	kubectl delete ${cleanup_spec}
	kubectl delete -f "tools/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml"

	popd
}
