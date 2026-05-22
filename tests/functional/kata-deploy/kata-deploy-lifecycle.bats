#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Kata Deploy Lifecycle Tests
#
# Validates kata-deploy behavior during DaemonSet restarts and uninstalls:
#
# 1. Artifacts present: After install, kata artifacts exist on the host,
#    RuntimeClasses are created, and the node is labeled.
#
# 2. Restart resilience: Running kata pods must survive a kata-deploy
#    DaemonSet restart without crashing. (Regression test for #12761)
#
# 3. Artifact cleanup: After helm uninstall, kata artifacts must be
#    fully removed from the host and containerd must remain healthy.
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

LIFECYCLE_POD_NAME="kata-lifecycle-test"

# Run a command on the host node's filesystem using a short-lived privileged pod.
# The host root is mounted at /host inside the pod.
# Usage: run_on_host "test -d /host/opt/kata && echo YES || echo NO"
#
# We avoid `kubectl run --rm -i` because rke2 injects session-recording banners
# into interactive pods, polluting stdout. Instead: create, wait, fetch logs, delete.
run_on_host() {
	local cmd="$1"
	local node_name
	node_name=$(kubectl get nodes --no-headers -o custom-columns=NAME:.metadata.name | head -1)
	local pod_name="host-exec-${RANDOM}"

	kubectl run "${pod_name}" \
		--image=quay.io/kata-containers/alpine-bash-curl:latest \
		--restart=Never \
		--overrides="{
			\"spec\": {
				\"nodeName\": \"${node_name}\",
				\"activeDeadlineSeconds\": 300,
				\"tolerations\": [{\"operator\": \"Exists\"}],
				\"containers\": [{
					\"name\": \"exec\",
					\"image\": \"quay.io/kata-containers/alpine-bash-curl:latest\",
					\"imagePullPolicy\": \"IfNotPresent\",
					\"command\": [\"sh\", \"-c\", \"${cmd}\"],
					\"securityContext\": {\"privileged\": true},
					\"volumeMounts\": [{\"name\": \"host\", \"mountPath\": \"/host\", \"readOnly\": true}]
				}],
				\"volumes\": [{\"name\": \"host\", \"hostPath\": {\"path\": \"/\"}}]
			}
		}" > /dev/null 2>&1

	local deadline=$((SECONDS + 60))
	while (( SECONDS < deadline )); do
		local phase
		phase=$(kubectl get pod "${pod_name}" -o jsonpath='{.status.phase}' 2>/dev/null) || true
		case "${phase}" in
			Succeeded|Failed) break ;;
		esac
		sleep 1
	done

	kubectl logs "${pod_name}" 2>/dev/null
	kubectl delete pod "${pod_name}" --ignore-not-found=true > /dev/null 2>&1
	[[ "${phase}" == "Succeeded" ]]
}

setup_file() {
	ensure_helm

	echo "# Image: ${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" >&3
	echo "# Hypervisor: ${KATA_HYPERVISOR}" >&3
	echo "# K8s distribution: ${KUBERNETES}" >&3
	echo "# Deploying kata-deploy..." >&3
	deploy_kata
	echo "# kata-deploy deployed successfully" >&3
}

@test "Kata artifacts are present on host after install" {
	echo "# Checking kata artifacts on host..." >&3

	run run_on_host "test -d /host/opt/kata && echo PRESENT || echo MISSING"
	echo "# /opt/kata directory: ${output}" >&3
	[[ "${output}" == *"PRESENT"* ]]

	run run_on_host "test -f /host/opt/kata/bin/containerd-shim-kata-v2 && echo FOUND || (test -f /host/opt/kata/runtime-rs/bin/containerd-shim-kata-v2 && echo FOUND || echo MISSING)"
	echo "# containerd-shim-kata-v2: ${output}" >&3
	[[ "${output}" == *"FOUND"* ]]

	# RuntimeClasses must exist (filter out AKS-managed ones)
	local rc_count
	rc_count=$(kubectl get runtimeclasses --no-headers 2>/dev/null | grep -v "kata-mshv-vm-isolation" | grep -c "kata" || true)
	echo "# Kata RuntimeClasses: ${rc_count}" >&3
	[[ ${rc_count} -gt 0 ]]

	# Node must have the kata-runtime label
	local label
	label=$(kubectl get nodes -o jsonpath='{.items[0].metadata.labels.katacontainers\.io/kata-runtime}')
	echo "# Node label katacontainers.io/kata-runtime: ${label}" >&3
	[[ "${label}" == "true" ]]
}

@test "DaemonSet restart does not crash running kata pods" {
	# Create a long-running kata pod
	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${LIFECYCLE_POD_NAME}
spec:
  runtimeClassName: kata-${KATA_HYPERVISOR}
  restartPolicy: Always
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  containers:
    - name: test
      image: quay.io/kata-containers/alpine-bash-curl:latest
      imagePullPolicy: IfNotPresent
      command: ["sleep", "infinity"]
EOF

	echo "# Waiting for kata pod to be running..." >&3
	kubectl wait --for=condition=Ready "pod/${LIFECYCLE_POD_NAME}" --timeout=120s

	# Record pod identity before the DaemonSet restart
	local pod_uid_before
	pod_uid_before=$(kubectl get pod "${LIFECYCLE_POD_NAME}" -o jsonpath='{.metadata.uid}')
	local restart_count_before
	restart_count_before=$(kubectl get pod "${LIFECYCLE_POD_NAME}" -o jsonpath='{.status.containerStatuses[0].restartCount}')
	echo "# Pod UID before: ${pod_uid_before}, restarts: ${restart_count_before}" >&3

	# Trigger a DaemonSet restart — this simulates what happens when a user
	# changes a label, updates a config value, or does a rolling update.
	echo "# Triggering kata-deploy DaemonSet restart..." >&3
	kubectl -n "${HELM_NAMESPACE}" rollout restart daemonset/kata-deploy

	echo "# Waiting for DaemonSet rollout to complete..." >&3
	kubectl -n "${HELM_NAMESPACE}" rollout status daemonset/kata-deploy --timeout=300s

	# On k3s/rke2 the new kata-deploy pod restarts the k3s service as
	# part of install, which causes a brief API server outage. Wait for
	# the node to become ready before querying pod status.
	kubectl wait nodes --timeout=120s --all --for condition=Ready=True
	echo "# Node is ready after DaemonSet rollout" >&3

	# The kata pod must still be Running with the same UID and no extra restarts.
	# Retry kubectl through any residual API unavailability.
	local pod_phase=""
	local retries=0
	while [[ ${retries} -lt 30 ]]; do
		pod_phase=$(kubectl get pod "${LIFECYCLE_POD_NAME}" -o jsonpath='{.status.phase}' 2>/dev/null) && break
		retries=$((retries + 1))
		sleep 2
	done
	echo "# Pod phase after restart: ${pod_phase}" >&3
	[[ "${pod_phase}" == "Running" ]]

	local pod_uid_after
	pod_uid_after=$(kubectl get pod "${LIFECYCLE_POD_NAME}" -o jsonpath='{.metadata.uid}')
	echo "# Pod UID after: ${pod_uid_after}" >&3
	[[ "${pod_uid_before}" == "${pod_uid_after}" ]]

	local restart_count_after
	restart_count_after=$(kubectl get pod "${LIFECYCLE_POD_NAME}" -o jsonpath='{.status.containerStatuses[0].restartCount}')
	echo "# Restart count after: ${restart_count_after}" >&3
	[[ "${restart_count_before}" == "${restart_count_after}" ]]

	echo "# SUCCESS: Kata pod survived DaemonSet restart without crashing" >&3
}

@test "Artifacts are fully cleaned up after uninstall" {
	echo "# Uninstalling kata-deploy..." >&3
	uninstall_kata
	echo "# Uninstall complete, verifying cleanup..." >&3

	# Wait for node to recover — containerd restart during cleanup may
	# cause brief unavailability (especially on k3s/rke2).
	kubectl wait nodes --timeout=120s --all --for condition=Ready=True

	# RuntimeClasses must be gone (filter out AKS-managed ones)
	local rc_count
	rc_count=$(kubectl get runtimeclasses --no-headers 2>/dev/null | grep -v "kata-mshv-vm-isolation" | grep -c "kata" || true)
	echo "# Kata RuntimeClasses remaining: ${rc_count}" >&3
	[[ ${rc_count} -eq 0 ]]

	# Node label must be removed
	local label
	label=$(kubectl get nodes -o jsonpath='{.items[0].metadata.labels.katacontainers\.io/kata-runtime}' 2>/dev/null || echo "")
	echo "# Node label after uninstall: '${label}'" >&3
	[[ -z "${label}" ]]

	# Kata artifacts must be removed from the host filesystem
	echo "# Checking host filesystem for leftover artifacts..." >&3
	run run_on_host "test -d /host/opt/kata && echo EXISTS || echo REMOVED"
	echo "# /opt/kata: ${output}" >&3
	[[ "${output}" == *"REMOVED"* ]]

	# Containerd must still be healthy and reporting a valid version.
	# After a CRI restart the kubelet may briefly report "Unknown" until it
	# re-probes the runtime, so retry for up to 60 seconds.
	local container_runtime_version=""
	local retries=0
	while [[ ${retries} -lt 30 ]]; do
		container_runtime_version=$(kubectl get nodes --no-headers -o custom-columns=CONTAINER_RUNTIME:.status.nodeInfo.containerRuntimeVersion)
		if [[ "${container_runtime_version}" != *"Unknown"* ]]; then
			break
		fi
		retries=$((retries + 1))
		sleep 2
	done
	echo "# Container runtime version: ${container_runtime_version}" >&3
	[[ "${container_runtime_version}" != *"Unknown"* ]]

	echo "# SUCCESS: All kata artifacts cleaned up, containerd healthy" >&3
}

teardown() {
	if [[ "${BATS_TEST_NAME}" == *"restart"* ]]; then
		kubectl delete pod "${LIFECYCLE_POD_NAME}" --ignore-not-found=true --wait=false 2>/dev/null || true
	fi
}

teardown_file() {
	kubectl delete pod "${LIFECYCLE_POD_NAME}" --ignore-not-found=true --wait=false 2>/dev/null || true
	uninstall_kata 2>/dev/null || true
}
