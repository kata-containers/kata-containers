#!/usr/bin/env bats
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Kata Deploy Functional Tests
#
# This test validates that kata-deploy successfully installs and configures
# Kata Containers on a Kubernetes cluster using Helm.
#
# Required environment variables:
#   DOCKER_REGISTRY - Container registry for kata-deploy image
#   DOCKER_REPO     - Repository name for kata-deploy image  
#   DOCKER_TAG      - Image tag to test
#   KATA_HYPERVISOR - Hypervisor to test (qemu, clh, etc.)
#   KUBERNETES      - K8s distribution (microk8s, k3s, rke2, etc.)
#
# Optional timeout configuration (increase for slow networks or large images):
#   KATA_DEPLOY_TIMEOUT                - Overall helm timeout (default: 30m)
#   KATA_DEPLOY_DAEMONSET_TIMEOUT      - DaemonSet rollout timeout in seconds (default: 1200 = 20m)
#                                        Includes time to pull kata-deploy image
#   KATA_DEPLOY_VERIFICATION_TIMEOUT   - Verification pod timeout in seconds (default: 180 = 3m)
#                                        Time for verification pod to run
#
# Example with custom timeouts for slow network:
#   KATA_DEPLOY_DAEMONSET_TIMEOUT=3600 bats kata-deploy.bats
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

# Load shared helm deployment helpers
source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

# Generate a verification pod YAML
# Arguments:
#   $1 - Pod name
# Output: Pod YAML to stdout
generate_verification_pod() {
	local pod_name="$1"
	
	cat <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: ${pod_name}
spec:
  runtimeClassName: kata-${KATA_HYPERVISOR}
  restartPolicy: Never
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  tolerations:
    - operator: Exists
  containers:
    - name: verify
      image: quay.io/kata-containers/alpine-bash-curl:latest
      imagePullPolicy: Always
      command:
        - sh
        - -c
        - |
          echo "=== Kata Verification ==="
          echo "Kernel: \$(uname -r)"
          echo "SUCCESS: Pod running with Kata runtime"
EOF
}

setup() {
	ensure_helm

	# We expect 2 runtime classes because:
	# * `kata` is the default runtimeclass created by Helm, basically an alias for `kata-${KATA_HYPERVISOR}`.
	# * `kata-${KATA_HYPERVISOR}` is the other one
	#    * As part of the tests we're only deploying the specific runtimeclass that will be used (via HELM_SHIMS), instead of all of them.
	#    * RuntimeClasses are now created by the Helm chart (runtimeClasses.enabled=true by default)
	expected_runtime_classes=2

	# We expect both runtime classes to have the same handler: kata-${KATA_HYPERVISOR}
	expected_handlers_re=( \
		"kata\s+kata-${KATA_HYPERVISOR}" \
		"kata-${KATA_HYPERVISOR}\s+kata-${KATA_HYPERVISOR}" \
	)
}

@test "Test runtimeclasses are being properly created and container runtime is not broken" {
	pushd "${repo_root_dir}"
	
	# Create verification pod spec using helper
	local verification_yaml
	verification_yaml=$(mktemp)
	generate_verification_pod "kata-deploy-verify" > "${verification_yaml}"
	
	# Install kata-deploy via Helm
	echo "Installing kata-deploy with Helm..."
	
	# Timeouts can be customized via environment variables:
	# - KATA_DEPLOY_TIMEOUT: Overall helm timeout (includes all hooks)
	#   Default: 600s (10 minutes)
	# - KATA_DEPLOY_DAEMONSET_TIMEOUT: Time to wait for kata-deploy DaemonSet rollout (image pull + pod start)
	#   Default: 300s (5 minutes) - accounts for large image downloads
	# - KATA_DEPLOY_VERIFICATION_TIMEOUT: Time to wait for verification pod to complete
	#   Default: 120s (2 minutes) - verification pod execution time
	local helm_timeout="${KATA_DEPLOY_TIMEOUT:-600s}"
	local daemonset_timeout="${KATA_DEPLOY_DAEMONSET_TIMEOUT:-300}"
	local verification_timeout="${KATA_DEPLOY_VERIFICATION_TIMEOUT:-120}"
	
	echo "Timeout configuration:"
	echo "  Helm overall: ${helm_timeout}"
	echo "  DaemonSet rollout: ${daemonset_timeout}s (includes image pull)"
	echo "  Verification pod: ${verification_timeout}s (pod execution)"

	# Deploy kata-deploy using shared helper with verification options
	HELM_TIMEOUT="${helm_timeout}" deploy_kata "" \
		--set-file verification.pod="${verification_yaml}" \
		--set verification.timeout="${verification_timeout}" \
		--set verification.daemonsetTimeout="${daemonset_timeout}"
	
	rm -f "${verification_yaml}"
	
	echo ""
	echo "::group::kata-deploy logs"
	kubectl -n kube-system logs --tail=200 -l name=kata-deploy
	echo "::endgroup::"

	echo ""
	echo "::group::Runtime classes"
	kubectl get runtimeclass
	echo "::endgroup::"
	
	# helm --wait already waits for post-install hooks to complete
	# If helm returns successfully, the verification job passed
	# The job is deleted after success (hook-delete-policy: hook-succeeded)
	echo ""
	echo "Helm install completed successfully - verification passed"
	
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
	
	popd
}

@test "Test node annotations are set after installation" {
	# Verify that kata-deploy sets the installation status annotations on nodes
	# These annotations are used by kata-upgrade to verify installation completion
	
	echo "Checking node annotations set by kata-deploy..."
	
	# Get nodes with kata-runtime label
	local kata_nodes
	kata_nodes=$(kubectl get nodes -l katacontainers.io/kata-runtime=true -o jsonpath='{.items[*].metadata.name}')
	
	[[ -n "${kata_nodes}" ]] || {
		echo "ERROR: No nodes found with katacontainers.io/kata-runtime=true label"
		return 1
	}
	
	echo "Found kata nodes: ${kata_nodes}"
	
	for node in ${kata_nodes}; do
		echo "Checking annotations on node: ${node}"
		
		# Check kata-deploy-installed-version annotation
		local installed_version
		installed_version=$(kubectl get node "${node}" -o jsonpath='{.metadata.annotations.katacontainers\.io/kata-deploy-installed-version}')
		
		[[ -n "${installed_version}" ]] || {
			echo "ERROR: Node ${node} missing annotation: katacontainers.io/kata-deploy-installed-version"
			kubectl get node "${node}" -o yaml | grep -A20 "annotations:"
			return 1
		}
		echo "  kata-deploy-installed-version: ${installed_version}"
		
		# Check kata-deploy-installed-at annotation (should be ISO 8601 timestamp)
		local installed_at
		installed_at=$(kubectl get node "${node}" -o jsonpath='{.metadata.annotations.katacontainers\.io/kata-deploy-installed-at}')
		
		[[ -n "${installed_at}" ]] || {
			echo "ERROR: Node ${node} missing annotation: katacontainers.io/kata-deploy-installed-at"
			kubectl get node "${node}" -o yaml | grep -A20 "annotations:"
			return 1
		}
		echo "  kata-deploy-installed-at: ${installed_at}"
		
		# Validate timestamp format (ISO 8601: YYYY-MM-DDTHH:MM:SS)
		[[ "${installed_at}" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2} ]] || {
			echo "ERROR: Invalid timestamp format: ${installed_at}"
			echo "Expected ISO 8601 format (YYYY-MM-DDTHH:MM:SS)"
			return 1
		}
		echo "  Timestamp format: valid"
	done
	
	echo ""
	echo "All node annotations verified successfully"
}

@test "Test annotations can be used to wait for installation" {
	# This test demonstrates using annotations to determine when installation is complete
	# instead of arbitrary sleep times
	
	echo "Demonstrating annotation-based installation detection..."
	
	# Get a kata node
	local node
	node=$(kubectl get nodes -l katacontainers.io/kata-runtime=true -o jsonpath='{.items[0].metadata.name}')
	
	[[ -n "${node}" ]] || {
		echo "ERROR: No kata nodes found"
		return 1
	}
	
	echo "Using node: ${node}"
	
	# Record current timestamp annotation
	local current_timestamp
	current_timestamp=$(kubectl get node "${node}" -o jsonpath='{.metadata.annotations.katacontainers\.io/kata-deploy-installed-at}')
	
	echo "Current installation timestamp: ${current_timestamp}"
	
	# In a real upgrade scenario, you would:
	# 1. Record the timestamp before triggering upgrade
	# 2. Trigger the upgrade (helm upgrade)
	# 3. Wait for the timestamp to change (proving new installation completed)
	# 4. Verify the version matches expected
	# 5. Then run verification pod
	
	# For this test, we just verify the current state is correct
	local installed_version
	installed_version=$(kubectl get node "${node}" -o jsonpath='{.metadata.annotations.katacontainers\.io/kata-deploy-installed-version}')
	
	echo "Installed version: ${installed_version}"
	
	# Deploy a verification pod only after confirming installation status
	echo ""
	echo "Deploying verification pod after confirming installation annotations..."
	
	local test_pod="kata-annotation-verify-$(date +%s)"
	generate_verification_pod "${test_pod}" | kubectl apply -n default -f -
	
	# Wait for pod to complete
	echo "Waiting for verification pod to complete..."
	kubectl wait pod "${test_pod}" --for=jsonpath='{.status.phase}'=Succeeded --timeout=180s || {
		echo "ERROR: Verification pod failed"
		kubectl describe pod "${test_pod}"
		kubectl logs "${test_pod}" || true
		kubectl delete pod "${test_pod}" --ignore-not-found
		return 1
	}
	
	echo ""
	echo "Verification pod logs:"
	kubectl logs "${test_pod}"
	
	# Cleanup
	kubectl delete pod "${test_pod}" --ignore-not-found
	
	echo ""
	echo "Annotation-based installation verification completed successfully"
}

teardown() {
	uninstall_kata
}
