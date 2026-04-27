#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Tests for kata-deploy kernel modules images feature
#
# Section 1: Helm template rendering tests (no cluster required)
# Section 2: E2E tests (require cluster with kata-deploy and module images on nodes)
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

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"
TEST_POD_NAME="kata-kernel-modules-verify"
MODULE_NTFS3_IMAGE_PATH="/opt/kata/share/kata-containers/kata-modules-ntfs.img"
MODULE_MLX5_IMAGE_PATH="/opt/kata/share/kata-containers/kata-modules-mlx5.img"

# =============================================================================
# Template Rendering Tests (no cluster required)
# =============================================================================

@test "Helm template: ConfigMap is created when kernelModulesImages is set on a shim" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${MODULES_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	grep -q "kind: ConfigMap" /tmp/rendered.yaml
	grep -q "kata-deploy-kernel-modules" /tmp/rendered.yaml
}

@test "Helm template: ConfigMap contains per-shim format" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${MODULES_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	grep -q "kernel-modules-images.list" /tmp/rendered.yaml
	# Format: shim:path:verity_params:modules
	grep -q "${KATA_HYPERVISOR:-qemu}:${MODULE_NTFS3_IMAGE_PATH}::ntfs3" /tmp/rendered.yaml
}

@test "Helm template: KERNEL_MODULES_IMAGES_ENABLED env var is set" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${MODULES_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	grep -q "KERNEL_MODULES_IMAGES_ENABLED" /tmp/rendered.yaml
	grep -A1 "KERNEL_MODULES_IMAGES_ENABLED" /tmp/rendered.yaml | grep -q '"true"'
}

@test "Helm template: kernel-modules-configs volume is mounted" {
	helm template kata-deploy "${CHART_PATH}" \
		-f "${MODULES_VALUES_FILE}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	grep -q "mountPath: /kernel-modules-configs/" /tmp/rendered.yaml
	grep -q "name: kernel-modules-configs" /tmp/rendered.yaml
}

@test "Helm template: No kernel modules resources when not configured" {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		> /tmp/rendered.yaml

	! grep -q "kata-deploy-kernel-modules" /tmp/rendered.yaml
	! grep -q "KERNEL_MODULES_IMAGES_ENABLED" /tmp/rendered.yaml
	! grep -q "kernel-modules-configs" /tmp/rendered.yaml
}

@test "Helm template: Multiple modules across shims render correctly" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
image:
  reference: quay.io/kata-containers/kata-deploy
  tag: latest
shims:
  qemu:
    kernelModulesImages:
      - path: "${MODULE_NTFS3_IMAGE_PATH}"
        verityParams: ""
        modules:
          - ntfs3
      - path: "${MODULE_MLX5_IMAGE_PATH}"
        verityParams: ""
        modules:
          - mlx5_core
          - mlx5_ib
  qemu-runtime-rs:
    kernelModulesImages:
      - path: "${MODULE_NTFS3_IMAGE_PATH}"
        verityParams: ""
        modules:
          - ntfs3
EOF

	helm template kata-deploy "${CHART_PATH}" -f "${values_file}" > /tmp/rendered.yaml
	rm -f "${values_file}"

	# Both shims should appear in the ConfigMap
	grep -q "qemu:${MODULE_NTFS3_IMAGE_PATH}::ntfs3" /tmp/rendered.yaml
	grep -q "qemu:${MODULE_MLX5_IMAGE_PATH}::mlx5_core,mlx5_ib" /tmp/rendered.yaml
	grep -q "qemu-runtime-rs:${MODULE_NTFS3_IMAGE_PATH}::ntfs3" /tmp/rendered.yaml
}

# =============================================================================
# End-to-End Tests (require cluster with kata-deploy)
# =============================================================================

@test "E2E: Kernel module is loaded when configured via Helm" {
	# Check that the module image exists on the node
	local node_name
	node_name=$(kubectl get nodes -l katacontainers.io/kata-runtime=true \
		--no-headers -o custom-columns=NAME:.metadata.name | head -1)
	[[ -n "${node_name}" ]] || skip "No kata-labeled node found"

	# Create a test pod
	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${TEST_POD_NAME}
spec:
  runtimeClassName: kata-${KATA_HYPERVISOR}
  restartPolicy: Never
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  containers:
    - name: test
      image: quay.io/kata-containers/alpine-bash-curl:latest
      command: ["sleep", "120"]
EOF

	echo "# Waiting for pod to be ready..." >&3
	local timeout=120
	local start_time
	start_time=$(date +%s)

	while true; do
		local phase
		phase=$(kubectl get pod "${TEST_POD_NAME}" -o jsonpath='{.status.phase}' 2>/dev/null || echo "Unknown")

		case "${phase}" in
			Running)
				echo "# Pod reached phase: ${phase}" >&3
				break
				;;
			Failed)
				echo "# Pod failed" >&3
				kubectl describe pod "${TEST_POD_NAME}" >&3
				die "Pod failed to run"
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

	# Verify ntfs3 module is loaded
	local modules_output
	modules_output=$(kubectl exec "${TEST_POD_NAME}" -- cat /proc/modules)
	echo "# /proc/modules output: ${modules_output}" >&3
	echo "${modules_output}" | grep -q "ntfs3"

	# Verify ntfs3 filesystem is registered
	local fs_output
	fs_output=$(kubectl exec "${TEST_POD_NAME}" -- cat /proc/filesystems)
	echo "# ntfs3 in /proc/filesystems: $(echo "${fs_output}" | grep ntfs3)" >&3
	echo "${fs_output}" | grep -q "ntfs3"

	echo "# SUCCESS: ntfs3 module loaded and filesystem registered" >&3
}

# =============================================================================
# Setup and Teardown
# =============================================================================

setup_file() {
	ensure_helm

	echo "# Hypervisor: ${KATA_HYPERVISOR}" >&3
	echo "# Image: ${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" >&3
	echo "# K8s distribution: ${KUBERNETES}" >&3

	# Deploy kata-deploy with kernel modules images configured
	export DEPLOY_VALUES_FILE
	DEPLOY_VALUES_FILE=$(mktemp)
	cat > "${DEPLOY_VALUES_FILE}" <<EOF
shims:
  ${KATA_HYPERVISOR}:
    kernelModulesImages:
      - path: "${MODULE_NTFS3_IMAGE_PATH}"
        verityParams: ""
        modules:
          - ntfs3
EOF

	echo "# Deploying kata-deploy with kernel modules images..." >&3
	deploy_kata "${DEPLOY_VALUES_FILE}"
	echo "# kata-deploy deployed successfully" >&3
}

setup() {
	# Create values file for template rendering tests
	MODULES_VALUES_FILE=$(mktemp)
	cat > "${MODULES_VALUES_FILE}" <<EOF
shims:
  ${KATA_HYPERVISOR:-qemu}:
    kernelModulesImages:
      - path: "${MODULE_NTFS3_IMAGE_PATH}"
        verityParams: ""
        modules:
          - ntfs3
EOF
}

teardown() {
	if [[ "${BATS_TEST_COMPLETED:-}" != "1" ]]; then
		echo "# Test failed, gathering diagnostics..." >&3
		kubectl describe pod "${TEST_POD_NAME}" 2>/dev/null || true
		echo "# kata-deploy logs:" >&3
		kubectl -n kube-system logs -l name=kata-deploy --tail=50 2>/dev/null || true
	fi

	kubectl delete pod "${TEST_POD_NAME}" --ignore-not-found=true --wait=false 2>/dev/null || true
	[[ -f "${MODULES_VALUES_FILE:-}" ]] && rm -f "${MODULES_VALUES_FILE}"
}

teardown_file() {
	echo "# Cleaning up..." >&3

	kubectl delete pod "${TEST_POD_NAME}" --ignore-not-found=true --wait=true --timeout=60s 2>/dev/null || true

	uninstall_kata
	[[ -f "${DEPLOY_VALUES_FILE:-}" ]] && rm -f "${DEPLOY_VALUES_FILE}"
}
