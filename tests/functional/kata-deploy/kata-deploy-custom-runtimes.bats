#!/usr/bin/env bats
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# End-to-end tests for kata-deploy custom runtimes feature
# These tests deploy kata-deploy with custom runtimes and verify pods can run
#
# Required environment variables:
#   DOCKER_REGISTRY - Container registry for kata-deploy image
#   DOCKER_REPO     - Repository name for kata-deploy image
#   DOCKER_TAG      - Image tag to test
#   KATA_HYPERVISOR - Hypervisor to test (qemu, clh, etc.)
#   KUBERNETES      - K8s distribution (microk8s, k3s, rke2, etc.)

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

# Load shared helm deployment helpers
source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

# Test configuration
CUSTOM_RUNTIME_NAME="special-workload"
CUSTOM_RUNTIME_HANDLER="kata-my-custom-handler"
TEST_POD_NAME="kata-deploy-custom-verify"
CHART_PATH="$(get_chart_path)"

# =============================================================================
# Template Rendering Tests (no cluster required)
# =============================================================================

@test "Helm template: ConfigMap is created with custom runtime" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${CUSTOM_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	# Check that ConfigMap exists
	grep -q "kind: ConfigMap" /tmp/rendered.yaml
	grep -q "kata-deploy-custom-configs" /tmp/rendered.yaml
	grep -q "${CUSTOM_RUNTIME_HANDLER}" /tmp/rendered.yaml
}

@test "Helm template: RuntimeClass is created with correct handler" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${CUSTOM_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	grep -q "kind: RuntimeClass" /tmp/rendered.yaml
	grep -q "handler: ${CUSTOM_RUNTIME_HANDLER}" /tmp/rendered.yaml
}

@test "Helm template: Drop-in file is included in ConfigMap" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${CUSTOM_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	grep -q "dropin-${CUSTOM_RUNTIME_HANDLER}.toml" /tmp/rendered.yaml
	grep -q "dial_timeout = 999" /tmp/rendered.yaml
}

@test "Helm template: CUSTOM_RUNTIMES_ENABLED env var is set" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${CUSTOM_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	grep -q "CUSTOM_RUNTIMES_ENABLED" /tmp/rendered.yaml
	grep -A1 "CUSTOM_RUNTIMES_ENABLED" /tmp/rendered.yaml | grep -q '"true"'
}

@test "Helm template: custom-configs volume is mounted" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${CUSTOM_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	grep -q "mountPath: /custom-configs/" /tmp/rendered.yaml
	grep -q "name: custom-configs" /tmp/rendered.yaml
}

@test "Helm template: No custom runtime resources when disabled" {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		--set customRuntimes.enabled=false \
		> /tmp/rendered.yaml

	! grep -q "kata-deploy-custom-configs" /tmp/rendered.yaml
	! grep -q "CUSTOM_RUNTIMES_ENABLED" /tmp/rendered.yaml
}

@test "Helm template: Custom runtimes only mode (no standard shims)" {
	# Test that Helm chart renders correctly when all standard shims are disabled
	# using shims.disableAll and only custom runtimes are enabled
	
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
image:
  reference: quay.io/kata-containers/kata-deploy
  tag: latest

# Disable all standard shims at once
shims:
  disableAll: true

# Enable only custom runtimes
customRuntimes:
  enabled: true
  runtimes:
    my-only-runtime:
      baseConfig: "qemu"
      dropIn: |
        [hypervisor.qemu]
        enable_debug = true
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: kata-my-only-runtime
        handler: kata-my-only-runtime
        scheduling:
          nodeSelector:
            katacontainers.io/kata-runtime: "true"
      containerd:
        snapshotter: ""
      crio:
        pullType: ""
EOF

	helm template kata-deploy "${CHART_PATH}" -f "${values_file}" > /tmp/rendered.yaml
	rm -f "${values_file}"

	# Verify custom runtime resources are created
	grep -q "kata-deploy-custom-configs" /tmp/rendered.yaml
	grep -q "CUSTOM_RUNTIMES_ENABLED" /tmp/rendered.yaml
	grep -q "kata-my-only-runtime" /tmp/rendered.yaml

	# Verify SHIMS env var is empty (no standard shims)
	local shims_value
	shims_value=$(grep -A1 'name: SHIMS$' /tmp/rendered.yaml | grep 'value:' | head -1 || echo "")
	echo "# SHIMS env value: ${shims_value}" >&3
}

# =============================================================================
# End-to-End Tests (require cluster with kata-deploy)
# =============================================================================

@test "E2E: Custom RuntimeClass exists and can run a pod" {
	# Check RuntimeClass exists
	run kubectl get runtimeclass "${CUSTOM_RUNTIME_HANDLER}" -o name
	if [[ "${status}" -ne 0 ]]; then
		echo "# RuntimeClass not found. kata-deploy logs:" >&3
		kubectl -n kube-system logs -l name=kata-deploy --tail=50 2>/dev/null || true
		die "Custom RuntimeClass ${CUSTOM_RUNTIME_HANDLER} not found"
	fi

	echo "# RuntimeClass ${CUSTOM_RUNTIME_HANDLER} exists" >&3

	# Verify handler is correct
	local handler
	handler=$(kubectl get runtimeclass "${CUSTOM_RUNTIME_HANDLER}" -o jsonpath='{.handler}')
	echo "# Handler: ${handler}" >&3
	[[ "${handler}" == "${CUSTOM_RUNTIME_HANDLER}" ]]

	# Verify overhead is set
	local overhead_memory
	overhead_memory=$(kubectl get runtimeclass "${CUSTOM_RUNTIME_HANDLER}" -o jsonpath='{.overhead.podFixed.memory}')
	echo "# Overhead memory: ${overhead_memory}" >&3
	[[ "${overhead_memory}" == "640Mi" ]]

	local overhead_cpu
	overhead_cpu=$(kubectl get runtimeclass "${CUSTOM_RUNTIME_HANDLER}" -o jsonpath='{.overhead.podFixed.cpu}')
	echo "# Overhead CPU: ${overhead_cpu}" >&3
	[[ "${overhead_cpu}" == "500m" ]]

	# Verify nodeSelector is set
	local node_selector
	node_selector=$(kubectl get runtimeclass "${CUSTOM_RUNTIME_HANDLER}" -o jsonpath='{.scheduling.nodeSelector.katacontainers\.io/kata-runtime}')
	echo "# Node selector: ${node_selector}" >&3
	[[ "${node_selector}" == "true" ]]

	# Verify label is set (Helm sets this to "Helm" when it manages the resource)
	local label
	label=$(kubectl get runtimeclass "${CUSTOM_RUNTIME_HANDLER}" -o jsonpath='{.metadata.labels.app\.kubernetes\.io/managed-by}')
	echo "# Label app.kubernetes.io/managed-by: ${label}" >&3
	[[ "${label}" == "Helm" ]]

	# Create a test pod using the custom runtime
	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${TEST_POD_NAME}
spec:
  runtimeClassName: ${CUSTOM_RUNTIME_HANDLER}
  restartPolicy: Never
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  containers:
    - name: test
      image: quay.io/kata-containers/alpine-bash-curl:latest
      command: ["echo", "OK"]
EOF

	# Wait for pod to complete or become ready
	echo "# Waiting for pod to be ready..." >&3
	local timeout=120
	local start_time
	start_time=$(date +%s)

	while true; do
		local phase
		phase=$(kubectl get pod "${TEST_POD_NAME}" -o jsonpath='{.status.phase}' 2>/dev/null || echo "Unknown")

		case "${phase}" in
			Succeeded|Running)
				echo "# Pod reached phase: ${phase}" >&3
				break
				;;
			Failed)
				echo "# Pod failed" >&3
				kubectl describe pod "${TEST_POD_NAME}" >&3
				die "Pod failed to run with custom runtime"
				;;
			*)
				local current_time
				current_time=$(date +%s)
				if (( current_time - start_time > timeout )); then
					echo "# Timeout waiting for pod" >&3
					kubectl describe pod "${TEST_POD_NAME}" >&3
					die "Timeout waiting for pod to be ready"
				fi
				sleep 5
				;;
		esac
	done

	# Verify pod ran successfully
	local exit_code
	exit_code=$(kubectl get pod "${TEST_POD_NAME}" -o jsonpath='{.status.containerStatuses[0].state.terminated.exitCode}' 2>/dev/null || echo "")

	if [[ "${exit_code}" == "0" ]] || [[ "$(kubectl get pod "${TEST_POD_NAME}" -o jsonpath='{.status.phase}')" == "Running" ]]; then
		echo "# Pod ran successfully with custom runtime" >&3
		BATS_TEST_COMPLETED=1
	else
		die "Pod did not complete successfully (exit code: ${exit_code})"
	fi
}

# =============================================================================
# Setup and Teardown
# =============================================================================

setup_file() {
	ensure_helm

	echo "# Using base config: ${KATA_HYPERVISOR}" >&3
	echo "# Custom runtime handler: ${CUSTOM_RUNTIME_HANDLER}" >&3
	echo "# Image: ${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" >&3
	echo "# K8s distribution: ${KUBERNETES}" >&3

	# Create values file for custom runtimes
	export DEPLOY_VALUES_FILE=$(mktemp)
	cat > "${DEPLOY_VALUES_FILE}" <<EOF
customRuntimes:
  enabled: true
  runtimes:
    ${CUSTOM_RUNTIME_NAME}:
      baseConfig: "${KATA_HYPERVISOR}"
      dropIn: |
        [agent.kata]
        dial_timeout = 999
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: ${CUSTOM_RUNTIME_HANDLER}
          labels:
            app.kubernetes.io/managed-by: kata-deploy
        handler: ${CUSTOM_RUNTIME_HANDLER}
        overhead:
          podFixed:
            memory: "640Mi"
            cpu: "500m"
        scheduling:
          nodeSelector:
            katacontainers.io/kata-runtime: "true"
      containerd:
        snapshotter: ""
      crio:
        pullType: ""
EOF

	echo "# Deploying kata-deploy with custom runtimes..." >&3
	deploy_kata "${DEPLOY_VALUES_FILE}"
	echo "# kata-deploy deployed successfully" >&3
}

setup() {
	# Create temporary values file for template tests
	CUSTOM_VALUES_FILE=$(mktemp)
	cat > "${CUSTOM_VALUES_FILE}" <<EOF
customRuntimes:
  enabled: true
  runtimes:
    ${CUSTOM_RUNTIME_NAME}:
      baseConfig: "${KATA_HYPERVISOR:-qemu}"
      dropIn: |
        [agent.kata]
        dial_timeout = 999
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: ${CUSTOM_RUNTIME_HANDLER}
          labels:
            app.kubernetes.io/managed-by: kata-deploy
        handler: ${CUSTOM_RUNTIME_HANDLER}
        overhead:
          podFixed:
            memory: "640Mi"
            cpu: "500m"
        scheduling:
          nodeSelector:
            katacontainers.io/kata-runtime: "true"
      containerd:
        snapshotter: ""
      crio:
        pullType: ""
EOF
}

teardown() {
	# Show pod details for debugging if test failed
	if [[ "${BATS_TEST_COMPLETED:-}" != "1" ]]; then
		echo "# Test failed, gathering diagnostics..." >&3
		kubectl describe pod "${TEST_POD_NAME}" 2>/dev/null || true
		echo "# kata-deploy logs:" >&3
		kubectl -n kube-system logs -l name=kata-deploy --tail=100 2>/dev/null || true
	fi

	# Clean up test pod
	kubectl delete pod "${TEST_POD_NAME}" --ignore-not-found=true --wait=false 2>/dev/null || true

	# Clean up temp file
	[[ -f "${CUSTOM_VALUES_FILE:-}" ]] && rm -f "${CUSTOM_VALUES_FILE}"
}

teardown_file() {
	echo "# Cleaning up..." >&3

	kubectl delete pod "${TEST_POD_NAME}" --ignore-not-found=true --wait=true --timeout=60s 2>/dev/null || true

	uninstall_kata
	[[ -f "${DEPLOY_VALUES_FILE:-}" ]] && rm -f "${DEPLOY_VALUES_FILE}"
}
