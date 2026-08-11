#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for who holds which credentials. No cluster required.
#
# Job mode's claim over the DaemonSet is that the privileged pods - the ones that
# run on every node Kata is installed on, root on the host, next to whatever else
# that node runs - hold no API credentials at all. Everything that needs the
# apiserver is done by the unprivileged dispatcher, which an operator can pin to
# trusted nodes.
#
# That claim is only worth as much as the rendered manifests, and it is easy to
# undo by accident: adding a ServiceAccount to a per-node Job, or reintroducing a
# stage that talks to the apiserver, would both look harmless in review. These
# tests fail when that happens.

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

# Assert that rendered output does NOT contain a pattern. `! grep ...` would be
# exempt from set -e, so as anything but the last line of a test it could never
# fail one.
refute_match() {
	local rendered="${1}" pattern="${2}"
	if echo "${rendered}" | grep -q -- "${pattern}"; then
		echo "unexpected in rendered output: ${pattern}" >&2
		return 1
	fi
}

@test "Helm template (job mode): the pods that run on every node carry no token" {
	local jobs
	jobs=$(per_node_jobs)

	# A ServiceAccount here would be readable by root on the node, and these pods
	# are privileged on purpose - so there is deliberately nothing to read.
	refute_match "${jobs}" 'serviceAccountName'
	# Kubernetes mounts the namespace's default ServiceAccount unless told not to,
	# so omitting serviceAccountName is not by itself enough.
	[[ "$(echo "${jobs}" | grep -c 'automountServiceAccountToken: false')" -eq 2 ]]
}

@test "Helm template (job mode): no stage inside a per-node Job talks to the apiserver" {
	local jobs
	jobs=$(per_node_jobs)

	# The label/unlabel stages are the API-using ones; they now happen in the
	# dispatcher. Anything reintroducing them here would need a token again.
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
	local install cleanup
	install=$(render_job_mode kata-deploy-install-job.yaml \
		--set 'startupTaints[0]=kata/installing:NoSchedule')
	cleanup=$(render_job_mode kata-deploy-cleanup-job.yaml)

	# Labelling last, and only once the node is Ready again: the install restarts
	# the node's CRI runtime, and the label is what admits workloads.
	echo "${install}" | grep -q -- '--node-label=true'
	echo "${install}" | grep -q -- '--wait-node-ready-secs=300'
	echo "${install}" | grep -q -- '--remove-node-taints=kata/installing:NoSchedule'

	# Cleanup goes the other way: unlabel before the node is taken apart.
	echo "${cleanup}" | grep -q -- '--remove-node-label'
	refute_match "${cleanup}" '--node-label='
}

@test "Helm template (job mode): the dispatcher may patch nodes, and nothing else may" {
	local rbac
	rbac=$(render_job_mode kata-rbac.yaml)

	# Exactly one rule over nodes, on the dispatcher's own ClusterRole.
	echo "${rbac}" | grep -q 'verbs: \["list", "get", "patch"\]'
	[[ "$(echo "${rbac}" | grep -c 'resources: \["nodes"\]')" -eq 1 ]]
}

@test "Helm template (job mode): kubelet config is only read when it matters" {
	local rbac install
	rbac=$(render_job_mode kata-rbac.yaml)
	install=$(render_job_mode kata-deploy-install-job.yaml)

	# The default enables the confidential shims, which pull images inside
	# CreateContainer - the one thing slow enough to hit runtimeRequestTimeout.
	echo "${rbac}" | grep -q 'resources: \["nodes/proxy"\]'
	echo "${install}" | grep -q -- '--kubelet-timeout-warn-secs=600'

	# Without guest pull or image conversion the warning is pointless, and the
	# grant that goes with it - reading any node's kubelet configuration - is not
	# asked for.
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
