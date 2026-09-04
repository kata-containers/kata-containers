#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for how a failed rollout explains itself. No cluster
# required.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"

render_job_mode() {
	local template="${1}"
	shift
	helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--show-only "templates/${template}" \
		"$@"
}

@test "Helm template (job mode): every stage keeps its last words in the pod's status" {
	local jobs stages daemonset
	jobs=$(render_job_mode kata-deploy-job-templates.yaml)

	stages=$(echo "${jobs}" | grep -c 'command: \["/usr/bin/kata-deploy"')
	[[ "${stages}" -gt 0 ]]
	[[ "$(echo "${jobs}" | grep -c 'terminationMessagePolicy: FallbackToLogsOnError')" -eq "${stages}" ]]

	# The DaemonSet runs the same binary, and restarts scroll its log away.
	daemonset=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=daemonset \
		--show-only templates/kata-deploy.yaml)
	echo "${daemonset}" | grep -q 'terminationMessagePolicy: FallbackToLogsOnError'
}

@test "Helm template (job mode): the dispatcher's own summary survives its pod" {
	local template rendered
	# Without this, a dispatcher that was OOM-killed leaves a pod whose status
	# says "Error" and nothing else.
	for template in kata-deploy-install-job.yaml kata-deploy-cleanup-job.yaml; do
		rendered=$(render_job_mode "${template}")
		echo "${rendered}" | grep -q 'terminationMessagePolicy: FallbackToLogsOnError'
	done

	rendered=$(render_job_mode kata-deploy-reconcile.yaml --set job.reconcile.enabled=true)
	echo "${rendered}" | grep -q 'terminationMessagePolicy: FallbackToLogsOnError'
}
