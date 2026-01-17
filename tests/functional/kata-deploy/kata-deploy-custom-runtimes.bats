#!/usr/bin/env bats
#
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# Tests for custom runtimes feature in kata-deploy.
# These tests validate the baseConfig + dropIn approach for custom runtimes.

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

# Custom runtime name used in tests
CUSTOM_RUNTIME_NAME="my-custom-runtime"
CUSTOM_RUNTIME_CLASS="kata-${CUSTOM_RUNTIME_NAME}"

setup_file() {
	ensure_yq

	pushd "${repo_root_dir}"

	# Set the latest image
	export HELM_IMAGE_REFERENCE="${DOCKER_REGISTRY}/${DOCKER_REPO}"
	export HELM_IMAGE_TAG="${DOCKER_TAG}"

	# Enable debug for Kata Containers
	export HELM_DEBUG="true"

	# We need at least one standard shim for kata-deploy to work
	export HELM_SHIMS="${KATA_HYPERVISOR}"
	export HELM_DEFAULT_SHIM="${KATA_HYPERVISOR}"

	# Let the Helm chart create the default runtime class
	export HELM_CREATE_DEFAULT_RUNTIME_CLASS="true"

	HOST_OS=""
	if [[ "${KATA_HOST_OS}" = "cbl-mariner" ]]; then
		HOST_OS="${KATA_HOST_OS}"
	fi
	export HELM_HOST_OS="${HOST_OS}"

	export HELM_K8S_DISTRIBUTION="${KUBERNETES}"

	# Create a custom values file with custom runtimes enabled
	export HELM_EXTRA_VALUES_FILE=$(mktemp)
	cat > "${HELM_EXTRA_VALUES_FILE}" << EOF
customRuntimes:
  enabled: true
  runtimes:
    ${CUSTOM_RUNTIME_NAME}:
      baseConfig: "qemu"
      dropIn: |
        [hypervisor.qemu]
        default_memory = 512
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: ${CUSTOM_RUNTIME_CLASS}
          labels:
            app.kubernetes.io/managed-by: kata-deploy
        handler: ${CUSTOM_RUNTIME_CLASS}
        overhead:
          podFixed:
            memory: "320Mi"
            cpu: "250m"
        scheduling:
          nodeSelector:
            katacontainers.io/kata-runtime: "true"
EOF

	# Deploy kata-deploy with custom runtimes using helm_helper
	helm_helper

	echo "::group::kata-deploy logs"
	kubectl -n kube-system logs --tail=100 -l name=kata-deploy
	echo "::endgroup::"

	echo "::group::Runtime classes"
	kubectl get runtimeclass
	echo "::endgroup::"

	popd
}

setup() {
	# No per-test setup needed
	:
}

@test "Custom runtime RuntimeClass is created" {
	# Verify the custom RuntimeClass exists
	kubectl get runtimeclass "${CUSTOM_RUNTIME_CLASS}"

	# Verify the handler is correct
	handler=$(kubectl get runtimeclass "${CUSTOM_RUNTIME_CLASS}" -o jsonpath='{.handler}')
	[[ "${handler}" == "${CUSTOM_RUNTIME_CLASS}" ]]
}

@test "Custom runtime can run a pod" {
	# Create a test pod using the custom runtime
	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: custom-runtime-test-pod
spec:
  runtimeClassName: ${CUSTOM_RUNTIME_CLASS}
  containers:
  - name: test
    image: quay.io/prometheus/busybox:latest
    command: ["sleep", "30"]
EOF

	# Wait for pod to be ready
	kubectl wait --for=condition=Ready pod/custom-runtime-test-pod --timeout=120s

	# Verify pod is running with the custom runtime
	runtime_class=$(kubectl get pod custom-runtime-test-pod -o jsonpath='{.spec.runtimeClassName}')
	[[ "${runtime_class}" == "${CUSTOM_RUNTIME_CLASS}" ]]

	# Clean up
	kubectl delete pod custom-runtime-test-pod --wait=true
}

@test "Custom runtime config directory is created on node" {
	# Get a node name
	node_name=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')

	# Check that the custom runtime config directory exists
	# Using kubectl debug to run commands on the node
	kubectl debug node/"${node_name}" -it --image=busybox -- \
		ls /host/opt/kata/share/defaults/kata-containers/custom-runtimes/${CUSTOM_RUNTIME_CLASS}/
}

@test "Custom runtime drop-in file is created" {
	# Get a node name
	node_name=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')

	# Check that the drop-in file exists
	kubectl debug node/"${node_name}" -it --image=busybox -- \
		ls /host/opt/kata/share/defaults/kata-containers/custom-runtimes/${CUSTOM_RUNTIME_CLASS}/config.d/50-overrides.toml
}

@test "Nodes remain healthy after custom runtime deployment" {
	# Wait to see if the nodes get back into Ready state
	kubectl wait nodes --timeout=60s --all --for condition=Ready=True

	# Check that the container runtime version doesn't have unknown
	container_runtime_version=$(kubectl get nodes --no-headers -o custom-columns=CONTAINER_RUNTIME:.status.nodeInfo.containerRuntimeVersion)
	[[ ${container_runtime_version} != *"containerd://Unknown"* ]]
}

teardown() {
	# Clean up test pod if it still exists (per-test cleanup)
	kubectl delete pod custom-runtime-test-pod --ignore-not-found=true --wait=true || true
}

teardown_file() {
	pushd "${repo_root_dir}"
	helm uninstall kata-deploy --ignore-not-found --wait --cascade foreground --timeout 10m --namespace kube-system --debug
	popd

	rm -f "${HELM_EXTRA_VALUES_FILE}"
}
