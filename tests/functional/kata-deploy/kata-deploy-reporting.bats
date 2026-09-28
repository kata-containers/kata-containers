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

refute_match() {
	local rendered="${1}" pattern="${2}"
	if echo "${rendered}" | grep -q -- "${pattern}"; then
		echo "unexpected in rendered output: ${pattern}" >&2
		return 1
	fi
}

rbac_doc() {
	local rendered="${1}" name="${2}"
	echo "${rendered}" | awk -v want="  name: ${name}" '
		/^---$/ { if (found) { printf "%s", doc; exit } doc = ""; found = 0; next }
		{ doc = doc $0 "\n"; if ($0 == want) found = 1 }
		END { if (found) printf "%s", doc }
	'
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

@test "Helm template (job mode): the dispatcher may read the pods it has to explain" {
	local rbac role noderole
	rbac=$(render_job_mode kata-rbac.yaml)
	role=$(rbac_doc "${rbac}" 'kata-deploy-dispatcher-role')
	[[ -n "${role}" ]]

	# A Job's status says that it failed and nothing else; its pod holds the
	# stage, the exit code and the termination message.
	echo "${role}" | grep -q 'resources: \["pods"\]'
	# Namespaced: reading pods cluster-wide is a different thing to ask for.
	echo "${role}" | grep -q '^kind: Role$'

	noderole=$(rbac_doc "${rbac}" 'kata-deploy-dispatcher-noderole')
	refute_match "${noderole}" 'pods'
	# The node's own copy of the result rides on the runtime label's patch.
	echo "${noderole}" | grep -q 'verbs: \[.*"patch".*\]'

	refute_match "${role}" '"update"'
	refute_match "${role}" 'pods/exec'

	# The per-node Jobs still hold no credentials at all.
	refute_match "$(render_job_mode kata-deploy-job-templates.yaml)" 'serviceAccountName'
}

@test "Helm template (job mode): Events about the node are asked for where they live" {
	local rbac events binding
	rbac=$(render_job_mode kata-rbac.yaml)

	# The apiserver takes an Event about a cluster-scoped object in "default"
	# only, so asking in the release namespace grants nothing.
	events=$(rbac_doc "${rbac}" 'kata-deploy-dispatcher-events-role')
	[[ -n "${events}" ]]
	echo "${events}" | grep -q 'namespace: default'
	echo "${events}" | grep -q 'resources: \["events"\]'
	echo "${events}" | grep -q 'verbs: \["create"\]'
	refute_match "$(rbac_doc "${rbac}" 'kata-deploy-dispatcher-role')" 'events'

	# Reading or deleting somebody else's events is not part of reporting.
	refute_match "${events}" '"get"'
	refute_match "${events}" '"delete"'

	binding=$(rbac_doc "${rbac}" 'kata-deploy-dispatcher-events-rb')
	echo "${binding}" | grep -q 'namespace: default'
	echo "${binding}" | grep -q 'name: kata-deploy-dispatcher-sa'
}

@test "Helm template (job mode): a failed hook prints the dispatcher's summary" {
	local template rendered
	for template in kata-deploy-install-job.yaml kata-deploy-cleanup-job.yaml; do
		rendered=$(render_job_mode "${template}")
		echo "${rendered}" | grep -q '"helm.sh/hook-output-log-policy": hook-failed'
	done

	# Only on failure: a successful rollout's log is pages of per-node progress,
	# and printing it every time is how people learn to ignore it.
	refute_match "$(render_job_mode kata-deploy-install-job.yaml)" 'hook-output-log-policy": hook-succeeded'
}
