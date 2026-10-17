#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/../../gha-run-k8s-common.sh"
source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

setup_file() {
	case "$(uname -m)" in
	x86_64 | aarch64) ;;
	*) skip "the NVIDIA CPU profile supports amd64 and arm64" ;;
	esac
	ensure_helm
	ensure_yq
}

deploy_invalid_dcgm() {
	KATA_HYPERVISOR=qemu-nvidia-cpu-runtime-rs deploy_kata "" \
		--set deploymentMode=job --set debug=false \
		--set imagePullPolicy=IfNotPresent \
		--set shims.qemu-nvidia-cpu-runtime-rs.nvrc.enableDCGM=true \
		--set job.backoffLimit=0
}

@test "A CPU/DCGM install is rejected but its normal uninstall succeeds" {
	# No guest is started: this must fail at configuration validation, not
	# because the test node lacks GPU hardware or a CPU image artifact.
	run deploy_invalid_dcgm
	echo "${output}"
	[[ "${status}" -ne 0 ]]

	run kubectl -n "${HELM_NAMESPACE}" logs \
		-l "app.kubernetes.io/instance=${HELM_RELEASE_NAME},kata-deploy/stage=install" \
		-c load-kernel-modules --prefix
	echo "${output}"
	[[ "${status}" -eq 0 ]]
	[[ "${output}" == *"not an NVIDIA GPU shim"* ]]

	# Assert the first normal uninstall, not a helper's recovery/retry path.
	run helm uninstall "${HELM_RELEASE_NAME}" -n "${HELM_NAMESPACE}" \
		--wait --timeout "${HELM_TIMEOUT:-10m}"
	echo "${output}"
	[[ "${status}" -eq 0 ]]

	# Uninstall restarts the CRI on some distributions, temporarily taking
	# their control plane down with it. Preserve the first-uninstall assertion
	# above, but wait for the API to recover before querying Helm state.
	wait_for_nodes_ready

	run helm list -n "${HELM_NAMESPACE}" -q --filter "^${HELM_RELEASE_NAME}$"
	echo "${output}"
	[[ "${status}" -eq 0 ]]
	[[ -z "${output}" ]]
}

teardown() {
	if [[ "${BATS_TEST_COMPLETED:-}" != "1" && -z "${BATS_TEST_SKIPPED:-}" ]]; then
		kubectl -n "${HELM_NAMESPACE}" get jobs,pods -o wide || true
		kubectl -n "${HELM_NAMESPACE}" logs \
			-l "app.kubernetes.io/instance=${HELM_RELEASE_NAME},kata-deploy/stage=cleanup" \
			-c revert-cri --prefix || true
	fi
}

teardown_file() {
	uninstall_kata 2>/dev/null || true
}
