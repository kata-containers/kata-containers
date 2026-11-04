#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for the scheduled reconcile (job.reconcile). No cluster
# required.
#
# A tick has to install a node that joined after the release was applied, and do
# nothing else: it must not fight the rollout of an upgrade, must not tear a node
# down, and must reach the same nodes with the same labelling contract as the hook
# that installed the fleet. Every one of those is a flag, and a flag that quietly
# stops being rendered looks like nothing at all.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"

# Render one template of the chart in job mode with the reconcile enabled.
render_with_reconcile() {
	local template="${1}"
	shift
	helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set job.reconcile.enabled=true \
		--show-only "templates/${template}" \
		"$@"
}

render_reconcile() {
	render_with_reconcile kata-deploy-reconcile.yaml "$@"
}

# The install hook, for the flags a tick has to agree with.
render_install_hook() {
	helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--show-only templates/kata-deploy-install-job.yaml \
		"$@"
}

# Assert that rendered output does NOT contain a pattern. Not `! grep`, which
# fails without saying what it found.
refute_match() {
	local rendered="${1}" pattern="${2}"
	if grep -q -- "${pattern}" <<< "${rendered}"; then
		echo "unexpected in rendered output: ${pattern}" >&2
		return 1
	fi
}

# The dispatcher flags of a rendered manifest, one per line.
dispatcher_flags() {
	grep -o -- '--[a-z-]*[^"]*' | sed 's/[[:space:]]*$//' | sort -u
}

@test "Helm template: no reconcile unless it is asked for" {
	# A recurring privileged rollout is opt-in, so job mode alone renders nothing.
	local default_mode
	default_mode=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job)
	refute_match "${default_mode}" 'kind: CronJob'

	# And asking for it in daemonset mode asks for something that does not exist
	# there: that DaemonSet already installs a node the moment it appears.
	local daemonset_mode
	daemonset_mode=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=daemonset \
		--set job.reconcile.enabled=true)
	refute_match "${daemonset_mode}" 'kind: CronJob'
}

@test "Helm template: a tick covers new nodes and takes none apart" {
	local reconcile
	reconcile=$(render_reconcile)

	grep -q 'kind: CronJob' <<< "${reconcile}"
	grep -q -- '--job-template=/etc/kata-job/install-job.yaml' <<< "${reconcile}"

	# The install pipeline only. With a cleanup template the dispatcher would also
	# dismantle nodes that fell out of the selection, on a timer, with nobody
	# watching.
	refute_match "${reconcile}" '--cleanup-job-template'

	# Nodes that already show a finished install are left out, so a tick over a
	# settled fleet creates no pods at all.
	grep -q -- '--skip-satisfied-nodes' <<< "${reconcile}"

	# An upgrade rolling out while a tick starts is the overlap
	# concurrencyPolicy cannot see, and the tick is the one that gives way.
	grep -q -- '--yield-to-live-run' <<< "${reconcile}"
	grep -q 'concurrencyPolicy: Forbid' <<< "${reconcile}"

	# The Job a CronJob creates is named after the schedule it fired on, so a tick
	# cannot name its own Job to own the per-node Jobs with. It reads it off its pod.
	grep -q -- '--owner-job-from-pod=$(POD_NAME)' <<< "${reconcile}"
	grep -q 'fieldPath: metadata.name' <<< "${reconcile}"
	refute_match "${reconcile}" '--owner-job-name'
}

@test "Helm template: a tick waits for nothing and fails on nothing" {
	local reconcile
	reconcile=$(render_reconcile --set job.waitForNodesSeconds=120 \
		--set job.nodeSettleSeconds=30)

	# The hook's waits declare that nodes are expected and make an empty selection
	# an error. For something periodic that is a quiet no-op, so the wait is off -
	# which also switches off settling, since a tick that catches the fleet
	# mid-labelling can leave the rest to the next one.
	grep -q -- '--wait-for-nodes-secs=0' <<< "${reconcile}"
	refute_match "${reconcile}" '--wait-for-nodes-secs=120'
	refute_match "${reconcile}" '--node-settle-secs'

	# A tick that failed on a node is a failed Job to look at, not a retry loop:
	# the next tick is the retry.
	grep -q 'backoffLimit: 0' <<< "${reconcile}"
	grep -q 'failedJobsHistoryLimit: 3' <<< "${reconcile}"
}

@test "Helm template: a tick installs like the hook it stands in for" {
	local reconcile install
	reconcile=$(render_reconcile --set 'startupTaints[0]=kata/installing:NoSchedule')
	install=$(render_install_hook --set 'startupTaints[0]=kata/installing:NoSchedule')

	# Same Jobs, same nodes, same labelling contract. Anything the hook passes to
	# manage a node has to be passed here too, or a node this covers ends up
	# installed but never advertised, or advertised without the checks. Only the
	# four flags a tick differs by on purpose are left out, each covered by a test
	# of its own above.
	local flag
	while read -r flag; do
		case "${flag}" in
			--wait-for-nodes-secs=*|--node-settle-secs=*) continue ;;
			--cleanup-job-template=*|--owner-job-name=*) continue ;;
		esac
		grep -q -- "${flag}" <<< "${reconcile}"
	done < <(echo "${install}" | dispatcher_flags)

	# Including the name prefix, which is what makes the two runs one family: each
	# recognises the other's per-node Jobs instead of reading them as an abandoned
	# run's leftovers to clear away.
	grep -q -- '--name-prefix=kata-deploy-install' <<< "${reconcile}"

	# The pod that holds the token is still confined and unprivileged.
	grep -q 'runAsNonRoot: true' <<< "${reconcile}"
	grep -q 'readOnlyRootFilesystem: true' <<< "${reconcile}"
	grep -q 'privileged: false' <<< "${reconcile}"
}

@test "Helm template: the reconcile follows the dispatcher's node selection" {
	# nodeSelector, affinity terms and the NFD requirements the chart adds all
	# compile into --node-selector flags, and a tick that resolved a different set
	# than the hook would install a node the release never selected - or, worse,
	# leave out one it did.
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
nodeSelector:
  kata: allowed
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.kubernetes.io/instance-type
              operator: In
              values: ["m5.large"]
        - matchExpressions:
            - key: kata.io/pool
              operator: Exists
EOF

	local nfd reconcile install reconcile_selectors install_selectors
	for nfd in false true; do
		reconcile=$(render_reconcile -f "${values_file}" \
			--set "node-feature-discovery.enabled=${nfd}")
		install=$(render_install_hook -f "${values_file}" \
			--set "node-feature-discovery.enabled=${nfd}")

		reconcile_selectors=$(echo "${reconcile}" | grep -o -- '--node-selector=[^"]*' | sort -u)
		install_selectors=$(echo "${install}" | grep -o -- '--node-selector=[^"]*' | sort -u)
		[[ -n "${install_selectors}" ]]
		[[ "${reconcile_selectors}" == "${install_selectors}" ]]
	done
	rm -f "${values_file}"

	# Taints are the other half of the selection, and the dispatcher reads them off
	# the per-node Job template it is given. Both are given the same one, and only
	# uninstall is allowed to ignore a taint.
	refute_match "${reconcile}" '--ignore-node-taints'

	# An explicit node list is used verbatim in both, so naming nodes still means
	# those nodes and no others.
	local named
	named=$(render_reconcile --set 'job.nodes={worker-1,worker-2}')
	grep -q -- '--nodes=worker-1,worker-2' <<< "${named}"
	refute_match "${named}" '--node-selector='
}

@test "Helm template: uninstall stops the schedule before it reverts nodes" {
	local reconcile
	reconcile=$(render_reconcile)

	# Helm deletes the CronJob only once the pre-delete hooks are done, so a tick
	# firing in between would reinstall a node the uninstall has just cleaned. This
	# hook runs at a lower weight than the cleanup dispatcher's 5.
	grep -q 'helm.sh/hook": pre-delete' <<< "${reconcile}"
	grep -q 'helm.sh/hook-weight": "0"' <<< "${reconcile}"
	grep -q 'kubectl delete cronjob "kata-deploy-reconcile"' <<< "${reconcile}"

	# Only this release's per-node Jobs: with env.multiInstallSuffix a sibling
	# release's installs carry the same stage label, and they are none of this
	# uninstall's business.
	grep -q 'app.kubernetes.io/instance=kata-deploy' <<< "${reconcile}"
}

@test "Helm template: the reconcile asks for its own rights and no others" {
	local role
	role=$(render_with_reconcile kata-rbac.yaml)

	# Deleting the CronJob is what the uninstall hook needs, and get on pods is
	# how --owner-job-from-pod reads the Job that owns the tick.
	grep -q 'resources: \["cronjobs"\]' <<< "${role}"
	grep -q 'verbs: \["get", "list", "watch", "delete"\]' <<< "${role}"
	grep -q 'verbs: \["get", "list"\]' <<< "${role}"

	# A rollout only lists the pod a failed Job left behind.
	local plain
	plain=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--show-only templates/kata-rbac.yaml)
	grep -q 'resources: \["pods"\]' <<< "${plain}"
	grep -q 'verbs: \["list"\]' <<< "${plain}"
	refute_match "${plain}" 'cronjobs'

	# And nothing only the reconcile needs.
	grep -q 'verbs: \["create", "get", "list", "delete"\]' <<< "${plain}"
	refute_match "${plain}" 'watch'
}

@test "Helm template: the uninstall hook may watch what it waits for" {
	# kubectl answers a waited delete from an informer, so get and delete let
	# the object go while the wait spins on a forbidden list until --timeout.
	local hook role
	hook=$(render_with_reconcile kata-deploy-reconcile.yaml)
	role=$(render_with_reconcile kata-rbac.yaml)

	grep -q 'kubectl delete cronjob .* -n' <<< "${hook}"
	grep -q 'kubectl delete jobs -n' <<< "${hook}"
	# Two waited deletes, and no third one this test has not accounted for.
	[[ "$(echo "${hook}" | grep -c -- '--cascade=foreground --wait=true')" -eq 2 ]]

	# The first verbs: under a resources:, not a comment in between.
	local cronjob_verbs jobs_verbs
	cronjob_verbs=$(echo "${role}" | grep -A 4 'resources: \["cronjobs"\]' | grep -m1 'verbs:')
	jobs_verbs=$(echo "${role}" | grep -A 4 'resources: \["jobs"\]' | grep -m1 'verbs:')

	echo "# cronjobs: ${cronjob_verbs}" >&3
	echo "# jobs: ${jobs_verbs}" >&3
	for verb in list watch delete; do
		grep -q "\"${verb}\"" <<< "${cronjob_verbs}"
		grep -q "\"${verb}\"" <<< "${jobs_verbs}"
	done
}

@test "Helm template: a reconcile without a schedule is refused" {
	# An empty schedule renders a CronJob the apiserver rejects, and Helm reports
	# that as a validation error against the manifest rather than a missing value.
	run helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set job.reconcile.enabled=true \
		--set job.reconcile.schedule=""
	[ "${status}" -ne 0 ]
	grep -q 'job.reconcile.schedule is empty' <<< "${output}"
}
