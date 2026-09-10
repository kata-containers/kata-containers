#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Toggling the debug variant off has to withdraw its containerd runtime handler
# along with its RuntimeClass. A handler left registered names a configuration
# the same redeploy deleted, and pods that land on it never leave
# ContainerCreating.
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

BASE_HANDLER="kata-${KATA_HYPERVISOR}"
DEBUG_HANDLER="kata-${KATA_HYPERVISOR}-debug"
TOGGLE_POD_NAME="kata-variant-toggle"

# The handlers the CRI reports it can serve, one per line. This is the list the
# scheduler and the kubelet work from, so it is where a stale handler does harm.
node_runtime_handlers() {
	kubectl get nodes -o jsonpath='{range .items[*].status.runtimeHandlers[*]}{.name}{"\n"}{end}' 2>/dev/null
}

# Kubernetes below 1.30 does not publish the field at all.
runtime_handlers_published() {
	[[ -n "$(node_runtime_handlers)" ]]
}

# Wait for ${1} to be advertised (${2} = yes) or withdrawn (${2} = no). The
# kubelet republishes on its own sync interval, well after helm returns.
wait_for_handler() {
	local handler="${1}"
	local want="${2}"
	local retries=0

	while [[ ${retries} -lt 60 ]]; do
		local found="no"
		node_runtime_handlers | grep -qx "${handler}" && found="yes"
		[[ "${found}" == "${want}" ]] && return 0
		retries=$((retries + 1))
		sleep 2
	done

	echo "# handler ${handler}: wanted advertised=${want}, node reports:" >&3
	node_runtime_handlers >&3
	return 1
}

# Every containerd configuration kata-deploy may have written a handler into,
# drop-in or whole-file. Each distribution uses one, so the rest are absent.
CONTAINERD_CONFIG_DIRS="/host/etc/containerd \
/host/var/lib/rancher/k3s/agent/etc/containerd \
/host/var/lib/rancher/rke2/agent/etc/containerd \
/host/opt/kata/containerd"

# The containerd configuration files naming ${1}, or NONE.
containerd_configs_naming() {
	run_on_host "grep -rl ${1} ${CONTAINERD_CONFIG_DIRS} 2>/dev/null; echo NONE"
}

setup_file() {
	ensure_helm

	echo "# Image: ${DOCKER_REGISTRY}/${DOCKER_REPO}:${DOCKER_TAG}" >&3
	echo "# Hypervisor: ${KATA_HYPERVISOR}" >&3
	echo "# K8s distribution: ${KUBERNETES}" >&3

	# The base values carry debug: true, so this brings the variant up.
	echo "# Deploying kata-deploy with debug on..." >&3
	deploy_kata
	echo "# kata-deploy deployed successfully" >&3
}

@test "Debug variant is registered while debug is on" {
	kubectl get runtimeclass "${DEBUG_HANDLER}" -o name

	run run_on_host "test -d /host/opt/kata/share/defaults/kata-containers/custom-runtimes/${DEBUG_HANDLER} && echo PRESENT || echo MISSING"
	echo "# ${DEBUG_HANDLER} config directory: ${output}" >&3
	[[ "${output}" == *"PRESENT"* ]]

	# Also the baseline for the next test: without a hit here, finding no hit
	# once debug is off would say nothing about the handler having been removed.
	run containerd_configs_naming "${DEBUG_HANDLER}"
	echo "# containerd configs naming ${DEBUG_HANDLER}: ${output}" >&3
	[[ "${output}" == *"/host/"* ]]

	if ! runtime_handlers_published; then
		skip "this Kubernetes does not publish node.status.runtimeHandlers"
	fi

	wait_for_handler "${DEBUG_HANDLER}" yes
	wait_for_handler "${BASE_HANDLER}" yes
}

@test "Turning debug off withdraws the variant handler" {
	echo "# Redeploying with debug off..." >&3
	deploy_kata "" --set debug=false

	kubectl wait nodes --timeout=300s --all --for condition=Ready=True

	run kubectl get runtimeclass "${DEBUG_HANDLER}" -o name
	echo "# RuntimeClass ${DEBUG_HANDLER}: ${output}" >&3
	[[ "${status}" -ne 0 ]]

	run run_on_host "test -d /host/opt/kata/share/defaults/kata-containers/custom-runtimes/${DEBUG_HANDLER} && echo PRESENT || echo GONE"
	echo "# ${DEBUG_HANDLER} config directory: ${output}" >&3
	[[ "${output}" == *"GONE"* ]]

	# The configuration the handler would have been served from is gone, so the
	# handler has no business still being registered anywhere.
	run containerd_configs_naming "${DEBUG_HANDLER}"
	echo "# containerd configs naming ${DEBUG_HANDLER}: ${output}" >&3
	[[ "${output}" == "NONE" ]]

	# The base shim was never turned off, so a prune that took it too would be
	# just as broken.
	run containerd_configs_naming "${BASE_HANDLER}"
	echo "# containerd configs naming ${BASE_HANDLER}: ${output}" >&3
	[[ "${output}" == *"/host/"* ]]

	if ! runtime_handlers_published; then
		skip "this Kubernetes does not publish node.status.runtimeHandlers"
	fi

	wait_for_handler "${DEBUG_HANDLER}" no
	wait_for_handler "${BASE_HANDLER}" yes
}

@test "Base runtime still runs pods after the variant is withdrawn" {
	cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: ${TOGGLE_POD_NAME}
spec:
  runtimeClassName: ${BASE_HANDLER}
  restartPolicy: Never
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
  containers:
    - name: test
      image: quay.io/kata-containers/alpine-bash-curl:latest
      imagePullPolicy: IfNotPresent
      command: ["sleep", "60"]
EOF

	kubectl wait --for=condition=Ready "pod/${TOGGLE_POD_NAME}" --timeout=180s
}

teardown() {
	if [[ "${BATS_TEST_COMPLETED:-}" != "1" && -z "${BATS_TEST_SKIPPED:-}" ]]; then
		kubectl describe pod "${TOGGLE_POD_NAME}" 2>/dev/null || true
		kubectl -n "${HELM_NAMESPACE}" logs -l "$(kata_deploy_pod_selector)" 2>/dev/null || true
	fi
}

teardown_file() {
	kubectl delete pod "${TOGGLE_POD_NAME}" --ignore-not-found=true --wait=false 2>/dev/null || true
	uninstall_kata 2>/dev/null || true
}
