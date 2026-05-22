#!/bin/bash
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Shared helm deployment helpers for kata-deploy tests
#
# Required environment variables:
#   DOCKER_REGISTRY - Container registry for kata-deploy image
#   DOCKER_REPO     - Repository name for kata-deploy image
#   DOCKER_TAG      - Image tag to test
#   KATA_HYPERVISOR - Hypervisor to test (qemu, clh, etc.)
#   KUBERNETES      - K8s distribution (microk8s, k3s, rke2, etc.)

HELM_RELEASE_NAME="${HELM_RELEASE_NAME:-kata-deploy}"
HELM_NAMESPACE="${HELM_NAMESPACE:-kube-system}"

# Get the path to the helm chart
get_chart_path() {
	local script_dir
	script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
	echo "${script_dir}/../../../../tools/packaging/kata-deploy/helm-chart/kata-deploy"
}

# Generate base values YAML that disables all shims except the specified one
# Arguments:
#   $1 - Output file path
#   $2 - (Optional) Additional values file to merge
# shellcheck disable=SC2154
generate_base_values() {
	local output_file="$1"
	local extra_values_file="${2:-}"

	cat > "${output_file}" <<EOF
image:
  reference: ${DOCKER_REGISTRY}/${DOCKER_REPO}
  tag: ${DOCKER_TAG}

k8sDistribution: "${KUBERNETES}"
debug: true

# Disable all shims at once, then enable only the one we need
shims:
  disableAll: true
  ${KATA_HYPERVISOR}:
    enabled: true

defaultShim:
  amd64: ${KATA_HYPERVISOR}
  arm64: ${KATA_HYPERVISOR}

runtimeClasses:
  enabled: true
  createDefault: true
EOF
}

# Deploy kata-deploy using helm
# Arguments:
#   $1 - (Optional) Additional values file to merge with base values
#   $@ - (Optional) Additional helm arguments (after the first positional arg)
deploy_kata() {
	local extra_values_file="${1:-}"
	shift || true
	local extra_helm_args=("$@")

	local chart_path
	local values_yaml

	chart_path="$(get_chart_path)"
	values_yaml=$(mktemp)

	# Generate base values
	generate_base_values "${values_yaml}"

	# Add required helm repos for dependencies
	helm repo add node-feature-discovery https://kubernetes-sigs.github.io/node-feature-discovery/charts 2>/dev/null || true
	helm repo update

	# Build helm dependencies
	helm dependency build "${chart_path}"

	# Build helm command
	local helm_cmd=(
		helm upgrade --install "${HELM_RELEASE_NAME}" "${chart_path}"
		-f "${values_yaml}"
	)

	# Add extra values file if provided
	if [[ -n "${extra_values_file}" && -f "${extra_values_file}" ]]; then
		helm_cmd+=(-f "${extra_values_file}")
	fi

	# Add any extra helm arguments
	if [[ ${#extra_helm_args[@]} -gt 0 ]]; then
		helm_cmd+=("${extra_helm_args[@]}")
	fi

	helm_cmd+=(
		--namespace "${HELM_NAMESPACE}"
		--wait --timeout "${HELM_TIMEOUT:-10m}"
	)

	# Run helm install.
	# --wait makes helm block until all DaemonSet pods are Ready. The readiness
	# probe returns 200 only after install completes (artifacts extracted, CRI
	# restarted, node labeled), so no extra rollout/sleep polling is needed.
	#
	# Exception: on single-node clusters with maxUnavailable=1, helm --wait can
	# consider the DaemonSet ready with 0 ready pods. Belt-and-suspenders: also
	# kubectl wait on the pod readiness condition.
	"${helm_cmd[@]}"
	local ret=$?

	rm -f "${values_yaml}"

	if [[ ${ret} -ne 0 ]]; then
		echo "Helm install failed with exit code ${ret}" >&2
		return "${ret}"
	fi

	kubectl -n "${HELM_NAMESPACE}" wait pod -l name=kata-deploy \
		--for=condition=Ready --timeout="${HELM_TIMEOUT:-10m}" 2>/dev/null || true

	return 0
}

# Uninstall kata-deploy
uninstall_kata() {
	helm uninstall "${HELM_RELEASE_NAME}" -n "${HELM_NAMESPACE}" \
		--ignore-not-found --wait --cascade foreground --timeout 10m || true

	wait_for_api_and_retry_uninstall "${HELM_RELEASE_NAME}" "${HELM_NAMESPACE}"
}
