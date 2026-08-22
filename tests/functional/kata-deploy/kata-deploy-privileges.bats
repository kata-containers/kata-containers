#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for who holds which credentials. No cluster required.
#
# Job mode's claim is that the privileged pods, the ones running as root on every
# node Kata is installed on, hold no API credentials at all. The claim is only
# worth as much as the rendered manifests, and it is easy to undo by accident:
# adding a ServiceAccount to a per-node Job, or bringing back a stage that talks
# to the apiserver, would both look harmless in review.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"

# Render one template of the chart in job mode.
render_job_mode() {
	local template="${1}"
	shift
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		--set deploymentMode=job \
		--show-only "templates/${template}" \
		"$@"
}

# The per-node Job templates the dispatcher reads, as one YAML stream.
per_node_jobs() {
	render_job_mode kata-deploy-job-templates.yaml "$@"
}

# Assert that rendered output does NOT contain a pattern. Not `! grep`: set -e
# ignores its status, so anywhere but the last line of a test it could never fail
# one.
refute_match() {
	local rendered="${1}" pattern="${2}"
	if echo "${rendered}" | grep -q -- "${pattern}"; then
		echo "unexpected in rendered output: ${pattern}" >&2
		return 1
	fi
}

# The pod spec of one stage's per-node Job template, as it sits inside the
# job-templates ConfigMap.
#
# Grepping the whole ConfigMap cannot tell a pod-level key from a container-level
# one, and that is precisely the difference between a pod with a token and a pod
# without one.
per_node_pod_spec() {
	local stage="${1}"
	shift
	per_node_jobs "$@" | awk -v stage="${stage}" '
		$0 == "  " stage "-job.yaml: |" { inside = 1; next }
		inside && /^  [^ ]/ { inside = 0 }
		inside && $0 == "        spec:" { pod = 1; next }
		pod && /^        [^ ]/ { pod = 0 }
		pod { print }
	'
}

# One YAML document out of a rendered multi-document template, picked by name, so
# that a test can assert what a single ClusterRole grants rather than counting
# substrings across every document at once.
rbac_doc() {
	local rendered="${1}" name="${2}"
	echo "${rendered}" | awk -v want="  name: ${name}" '
		/^---$/ { if (found) { printf "%s", doc; exit } doc = ""; found = 0; next }
		{ doc = doc $0 "\n"; if ($0 == want) found = 1 }
		END { if (found) printf "%s", doc }
	'
}

@test "Helm template (job mode): the pods that run on every node carry no token" {
	local jobs
	jobs=$(per_node_jobs)

	# Root on the node can read whatever these pods mount, and they are privileged
	# on purpose, so there is deliberately nothing to read.
	refute_match "${jobs}" 'serviceAccountName'

	# Kubernetes mounts the namespace's default token unless told not to, so
	# omitting serviceAccountName is not enough on its own. The key only counts
	# where kubelet reads it, in each stage's pod spec.
	local stage pod_spec
	for stage in install cleanup; do
		pod_spec=$(per_node_pod_spec "${stage}")
		[[ -n "${pod_spec}" ]]
		echo "${pod_spec}" | grep -qE '^          automountServiceAccountToken: false$'
	done
}

@test "Helm template (job mode): nothing can keep a cleanup pod off a node" {
	# Pinning by name gets these pods past the scheduler, but a NoExecute taint
	# added since the install would evict a bound pod all the same, leaving the
	# node modified and out of reach. Hence one entry and nothing narrowing it: a
	# key or an effect would leave some taint untolerated.
	local cleanup_spec install_spec
	cleanup_spec=$(per_node_pod_spec cleanup)
	[[ "$(echo "${cleanup_spec}" | tolerations_of)" == "operator: Exists" ]]

	# The install is the opposite case: where it may land is the operator's call,
	# so its tolerations are theirs and none of them is a catch-all.
	install_spec=$(per_node_pod_spec install --set 'tolerations[0].key=kata' \
		--set 'tolerations[0].operator=Exists')
	echo "${install_spec}" | tolerations_of | grep 'key: kata' | grep -q 'operator: Exists'
	refute_match "$(echo "${install_spec}" | tolerations_of)" '^operator: Exists$'
}

@test "Helm template (job mode): no stage inside a per-node Job talks to the apiserver" {
	local jobs
	jobs=$(per_node_jobs)

	# The label and unlabel stages were the ones using the apiserver. Bringing
	# either back here would mean mounting a token again.
	refute_match "${jobs}" 'install-stage-label'
	refute_match "${jobs}" 'cleanup-stage-unlabel'

	# The stages that are left, in order. Install ends with the CRI restart, which
	# is why it is the main container rather than an init container.
	echo "${jobs}" | grep -q 'install-stage-host-check'
	echo "${jobs}" | grep -q 'install-stage-artifacts'
	echo "${jobs}" | grep -q 'install-stage-cri'
	echo "${jobs}" | grep -q 'cleanup-stage-revert-cri'
	echo "${jobs}" | grep -q 'cleanup-stage-remove-artifacts'
}

@test "Helm template (job mode): a per-node Job can tell which host it woke up on" {
	local stage pod_spec
	for stage in install cleanup; do
		pod_spec=$(per_node_pod_spec "${stage}")
		# A Job is bound to a node by name, and a name can outlive the machine that
		# answered to it. Without the host's own identity to compare against, the
		# machine ID the dispatcher passes down would be unverifiable.
		echo "${pod_spec}" | grep -q 'path: /etc/machine-id'
		echo "${pod_spec}" | grep -q 'mountPath: /host-machine-id'
	done
}

@test "Helm template (job mode): the privileged ServiceAccount is not created at all" {
	local rbac
	rbac=$(render_job_mode kata-rbac.yaml)

	# kata-deploy-sa can patch any node in the cluster and read any node's kubelet
	# configuration. In job mode nothing needs it, so it does not exist.
	refute_match "${rbac}" 'name: kata-deploy-sa$'
	refute_match "${rbac}" 'name: kata-deploy-role$'
	refute_match "${rbac}" 'name: kata-deploy-rb$'

	# Daemonset mode still needs it: that pod does the node-level work itself.
	local ds_rbac
	ds_rbac=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=daemonset \
		--show-only templates/kata-rbac.yaml)
	echo "${ds_rbac}" | grep -qE 'name: kata-deploy-sa$'
}

@test "Helm template (job mode): the dispatcher does the node-level work" {
	local install cleanup rendered
	install=$(render_job_mode kata-deploy-install-job.yaml \
		--set 'startupTaints[0]=kata/installing:NoSchedule')
	cleanup=$(render_job_mode kata-deploy-cleanup-job.yaml)

	for rendered in "${install}" "${cleanup}"; do
		echo "${rendered}" | grep -q 'image: ghcr.io/kata-containers/k8s-job-dispatcher:0.1.0'
		echo "${rendered}" | grep -q -- '/usr/bin/k8s-job-dispatcher'
		echo "${rendered}" | grep -q -- '--tracking-label-prefix=kata-deploy-job-dispatcher'
		echo "${rendered}" | grep -q -- '--node-label-key=katacontainers.io/kata-runtime'
		echo "${rendered}" | grep -q -- '--instance-label-prefix=kata-deploy.katacontainers.io'
		echo "${rendered}" | grep -q -- '--require-node-runtime-version'
		echo "${rendered}" | grep -q -- '--require-node-machine-id'
	done

	# Labelling last, and only once the node is Ready again: the install restarts
	# the node's CRI runtime, and the label is what admits workloads.
	echo "${install}" | grep -q -- '--node-label=true'
	echo "${install}" | grep -q -- '--wait-node-ready-secs=300'
	echo "${install}" | grep -q -- '--remove-node-taints=kata/installing:NoSchedule'

	# Claimed before its Job starts, so an install that dies half way still leaves
	# a node an uninstall can find.
	echo "${install}" | grep -q -- '--claim-node-pending'

	# Cleanup goes the other way: unlabel before the node is taken apart.
	echo "${cleanup}" | grep -q -- '--remove-node-label'
	refute_match "${cleanup}" '--node-label='
	refute_match "${cleanup}" '--claim-node-pending'
	refute_match "${cleanup}" '--require-node-handlers'
}

@test "Helm template (job mode): the dispatcher requires the handlers this release installs" {
	# The names must be the ones the chart registers RuntimeClasses for: waiting on
	# a handler no node will ever report fails every node.
	local suffix_args install flag_handlers rc_handlers
	for suffix_args in "" "--set env.multiInstallSuffix=dev"; do
		# shellcheck disable=SC2086
		install=$(render_job_mode kata-deploy-install-job.yaml ${suffix_args})
		flag_handlers=$(echo "${install}" |
			grep -o -- '--require-node-handlers=[^"]*' |
			sed 's/--require-node-handlers=//' | tr ',' '\n' | sort -u)
		[[ -n "${flag_handlers}" ]]

		# shellcheck disable=SC2086
		rc_handlers=$(helm template kata-deploy "${CHART_PATH}" \
			--set deploymentMode=job ${suffix_args} \
			--show-only templates/runtimeclasses.yaml |
			awk '/^handler: /{print $2}' | sort -u)

		[[ "${flag_handlers}" == "${rc_handlers}" ]]
	done

	# Custom runtimes are checked too, from the RuntimeClasses the user supplied.
	# Otherwise these nodes would be labelled on a stage's exit code alone.
	local values_file custom custom_flag custom_rc
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
shims:
  disableAll: true
snapshotter:
  setup: null
customRuntimes:
  enabled: true
  runtimes:
    my-gpu-runtime:
      baseConfig: "qemu"
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: kata-my-gpu-runtime
        handler: kata-my-gpu-runtime
EOF

	custom=$(render_job_mode kata-deploy-install-job.yaml -f "${values_file}")
	custom_flag=$(echo "${custom}" |
		grep -o -- '--require-node-handlers=[^"]*' |
		sed 's/--require-node-handlers=//' | tr ',' '\n' | sort -u)
	custom_rc=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job -f "${values_file}" \
		--show-only templates/custom-runtimes.yaml |
		awk '/^handler: /{print $2}' | sort -u)

	[[ "${custom_flag}" == "kata-my-gpu-runtime" ]]
	[[ "${custom_flag}" == "${custom_rc}" ]]
	rm -f "${values_file}"
}

@test "Helm template (job mode): the dispatcher may patch nodes, and nothing else may" {
	local rbac noderole
	rbac=$(render_job_mode kata-rbac.yaml)

	# Read that one document rather than the whole file, or a rule that drifted
	# into another role would still match.
	noderole=$(rbac_doc "${rbac}" 'kata-deploy-dispatcher-noderole')
	[[ -n "${noderole}" ]]
	echo "${noderole}" | grep -q 'resources: \["nodes"\]'
	echo "${noderole}" | grep -q 'verbs: \["list", "get", "patch"\]'

	# Job mode never reasons about DaemonSets: all releases in one cluster use the
	# same deployment mode.
	refute_match "${noderole}" 'daemonsets'
	refute_match "${noderole}" 'pods'
	refute_match "${noderole}" '"delete"'
	refute_match "${noderole}" '"create"'
	[[ "$(echo "${rbac}" | grep -c 'resources: \["nodes"\]')" -eq 1 ]]

	# Per-node Jobs are created and deleted, since a Job left by an earlier run has
	# to be replaced rather than adopted, but only in one namespace.
	local role
	role=$(rbac_doc "${rbac}" 'kata-deploy-dispatcher-role')
	[[ -n "${role}" ]]
	echo "${role}" | grep -q 'resources: \["jobs"\]'
	echo "${role}" | grep -q 'verbs: \["create", "get", "list", "delete"\]'
	echo "${role}" | grep -q '^kind: Role$'
}

@test "Helm template (job mode): kubelet config is only read when it matters" {
	local tee_values rbac install
	tee_values="${CHART_PATH}/try-kata-tee.values.yaml"
	rbac=$(render_job_mode kata-rbac.yaml -f "${tee_values}")
	install=$(render_job_mode kata-deploy-install-job.yaml -f "${tee_values}")

	# The TEE profile enables the confidential shims, which pull images inside
	# CreateContainer - the one thing slow enough to hit runtimeRequestTimeout.
	echo "${rbac}" | grep -q 'resources: \["nodes/proxy"\]'
	echo "${install}" | grep -q -- '--kubelet-timeout-warn-secs=600'

	# Without guest pull or image conversion there is nothing to warn about, so the
	# right to read any node's kubelet configuration is not asked for. Spelled out
	# rather than left to the chart defaults: what this asserts is that the rights
	# follow the configuration, not that the defaults happen to ask for neither.
	local plain_rbac plain_install
	plain_rbac=$(render_job_mode kata-rbac.yaml \
		--set shims.disableAll=true \
		--set shims.qemu.enabled=true \
		--set snapshotter.setup=null)
	plain_install=$(render_job_mode kata-deploy-install-job.yaml \
		--set shims.disableAll=true \
		--set shims.qemu.enabled=true \
		--set snapshotter.setup=null)

	refute_match "${plain_rbac}" 'nodes/proxy'
	refute_match "${plain_install}" '--kubelet-timeout-warn-secs'

	# A custom runtime asking for guest pull brings it back. Worth its own case:
	# customRuntimes.runtimes is a map, not a list, so the check that walks it is
	# easy to write in a way that silently matches nothing.
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
shims:
  disableAll: true
  qemu:
    enabled: true
snapshotter:
  setup: null
customRuntimes:
  enabled: true
  runtimes:
    guest-pulling-workload:
      baseConfig: "qemu"
      crio:
        pullType: "guest-pull"
EOF

	render_job_mode kata-deploy-install-job.yaml -f "${values_file}" |
		grep -q -- '--kubelet-timeout-warn-secs=600'
	render_job_mode kata-rbac.yaml -f "${values_file}" | grep -q 'nodes/proxy'
	rm -f "${values_file}"
}

@test "Helm template: neither mode grants the cluster-scoped rights the install shed" {
	# Advertising TEE keys and adjusting RuntimeClass overhead is the chart's job,
	# so no pod needs to write these any more, in either mode.
	local mode rbac
	for mode in job daemonset; do
		rbac=$(helm template kata-deploy "${CHART_PATH}" \
			--set "deploymentMode=${mode}" \
			--show-only templates/kata-rbac.yaml)

		refute_match "${rbac}" 'nodefeaturerules'
		refute_match "${rbac}" 'runtimeclasses'
		refute_match "${rbac}" 'customresourcedefinitions'
	done
}
