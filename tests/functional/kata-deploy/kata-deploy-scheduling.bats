#!/usr/bin/env bats
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for kata-deploy scheduling options (podLabels,
# podAnnotations, affinity). No cluster required.
#
# The pod-template metadata (podLabels, podAnnotations) is asserted in both
# deployment modes: on the DaemonSet pod template (deploymentMode: daemonset)
# and on the per-node install/cleanup Job pod templates (deploymentMode: job).
#
# Node selection comes from the same nodeSelector/affinity.nodeAffinity in both
# modes; in job mode it is compiled into the dispatcher's --node-selector flags,
# asserted at the end of this file. Pod-level affinity (podAffinity,
# podAntiAffinity) stays DaemonSet-only, because the dispatcher pins each per-node
# Job to a node via spec.template.spec.nodeName, making it a scheduling no-op.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"
RENDERED="/tmp/kata-deploy-scheduling-rendered.yaml"
RENDERED_JOBS="/tmp/kata-deploy-scheduling-rendered-jobs.yaml"

# Pinned to daemonset mode: every caller reads the DaemonSet back out, and job
# mode renders none.
render_chart() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		--set deploymentMode=daemonset \
		"$@" > "${RENDERED}"
}

# Render only the per-node Job templates ConfigMap (deploymentMode: job).
render_job_templates() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		--set deploymentMode=job \
		--show-only templates/kata-deploy-job-templates.yaml \
		"$@" > "${RENDERED_JOBS}"
}

# Render one of the job-mode dispatcher Jobs (stage: install|cleanup) and print
# the label selectors it will hand to the dispatcher, one per line. Node selection
# in job mode is expressed as repeated --node-selector flags, which the dispatcher
# unions - that is how nodeSelectorTerms keep their OR semantics.
dispatcher_selectors() {
	local stage="${1}"
	shift
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		--set deploymentMode=job \
		--show-only "templates/kata-deploy-${stage}-job.yaml" \
		"$@" |
		sed -n 's/^ *- "--node-selector=\(.*\)"$/\1/p'
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

# Extract one per-node Job manifest (stage: install|cleanup) from the rendered
# job-templates ConfigMap, stripping the 4-space block-scalar indentation so the
# result is a standalone Job manifest.
extract_pernode_job() {
	local stage="${1}"
	awk -v key="  ${stage}-job.yaml: |" '
		$0 == key { grab = 1; next }
		/^  [a-z-]+-job\.yaml: \|$/ { grab = 0 }
		grab { sub(/^    /, ""); print }
	' "${RENDERED_JOBS}"
}

# Extract the kata-deploy DaemonSet manifest (not kata-monitor or NFD subchart).
extract_kata_deploy_ds() {
	awk '
		/^kind: DaemonSet$/ { buf = $0 "\n"; in_ds = 1; has_name = 0; next }
		in_ds {
			buf = buf $0 "\n"
			if ($0 ~ /^  name: kata-deploy$/) { has_name = 1 }
			if ($0 ~ /^---$/) {
				if (has_name) { printf "%s", buf; exit }
				in_ds = 0; buf = ""; has_name = 0
				next
			}
		}
		END { if (has_name && in_ds) { printf "%s", buf } }
	' "${RENDERED}"
}

# Count nodeSelectorTerms under requiredDuringSchedulingIgnoredDuringExecution in a manifest.
count_required_node_selector_terms() {
	local manifest="${1}"
	echo "${manifest}" | awk '
		/requiredDuringSchedulingIgnoredDuringExecution:/ { in_req = 1; next }
		in_req && /preferredDuringSchedulingIgnoredDuringExecution:/ { exit }
		in_req && /^        [a-zA-Z]/ { exit }
		in_req && /- match(Expressions|Fields):/ { count++ }
		END { print count + 0 }
	'
}

# =============================================================================
# Template Rendering Tests (no cluster required)
# =============================================================================

@test "Helm template: default values keep single name pod label and no affinity" {
	render_chart

	local ds
	ds=$(extract_kata_deploy_ds)

	[[ -n "${ds}" ]]
	echo "${ds}" | grep -q "name: kata-deploy"
	echo "${ds}" | grep -A5 "template:" | grep -A3 "labels:" | grep -q "name: kata-deploy"
	! echo "${ds}" | grep -A10 "template:" | grep -A5 "metadata:" | grep -q "annotations:"
	! echo "${ds}" | grep -q "affinity:"
}

@test "Helm template: podLabels are applied to pod template" {
	render_chart --set podLabels.team=platform

	local ds
	ds=$(extract_kata_deploy_ds)

	echo "${ds}" | grep -A5 "template:" | grep -A4 "labels:" | grep -q "name: kata-deploy"
	echo "${ds}" | grep -A5 "template:" | grep -A4 "labels:" | grep -q "team: platform"
}

@test "Helm template: podLabels cannot override required name selector label" {
	render_chart --set podLabels.name=wrong

	local ds
	ds=$(extract_kata_deploy_ds)

	! echo "${ds}" | grep -A8 "template:" | grep -A6 "labels:" | grep -q "name: wrong"
	echo "${ds}" | grep -A8 "template:" | grep -A6 "labels:" | grep -q "name: kata-deploy"
	! echo "${ds}" | grep -A8 "template:" | grep -A6 "labels:" | grep "name:" | grep -qv "name: kata-deploy"
}

@test "Helm template: podAnnotations are applied to pod template" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
podAnnotations:
  example.com/owner: platform-team
  prometheus.io/scrape: "false"
EOF

	render_chart -f "${values_file}"
	rm -f "${values_file}"

	local ds
	ds=$(extract_kata_deploy_ds)

	echo "${ds}" | grep -A10 "template:" | grep -A5 "metadata:" | grep -q "annotations:"
	echo "${ds}" | grep -q "example.com/owner: platform-team"
	echo "${ds}" | grep -q 'prometheus.io/scrape: "false"'
}

@test "Helm template: user affinity is applied to pod spec" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - platform-team
  podAntiAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      - labelSelector:
          matchExpressions:
            - key: app
              operator: In
              values:
                - gpu-operator
        topologyKey: kubernetes.io/hostname
EOF

	render_chart -f "${values_file}"
	rm -f "${values_file}"

	local ds
	ds=$(extract_kata_deploy_ds)

	echo "${ds}" | grep -q "affinity:"
	echo "${ds}" | grep -q "node.cloud/reserved"
	echo "${ds}" | grep -q "platform-team"
	echo "${ds}" | grep -q "podAntiAffinity:"
	echo "${ds}" | grep -q "gpu-operator"
}

@test "Helm template: NFD enabled merges virtualization nodeAffinity with user nodeAffinity" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - platform-team
EOF

	render_chart -f "${values_file}" --set node-feature-discovery.enabled=true
	rm -f "${values_file}"

	local ds term_count
	ds=$(extract_kata_deploy_ds)
	term_count=$(count_required_node_selector_terms "${ds}")

	[[ "${term_count}" -eq 6 ]]
	echo "${ds}" | grep -q "node.cloud/reserved"
	echo "${ds}" | grep -q "platform-team"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.SVM"
}

@test "Helm template: NFD enabled applies virtualization nodeAffinity when user sets no affinity" {
	render_chart --set node-feature-discovery.enabled=true

	local ds term_count
	ds=$(extract_kata_deploy_ds)
	term_count=$(count_required_node_selector_terms "${ds}")

	[[ "${term_count}" -eq 6 ]]
	echo "${ds}" | grep -q "affinity:"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.SVM"
}

@test "Helm template: NFD merge preserves podAntiAffinity" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - platform-team
  podAntiAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      - labelSelector:
          matchExpressions:
            - key: app
              operator: In
              values:
                - gpu-operator
        topologyKey: kubernetes.io/hostname
EOF

	render_chart -f "${values_file}" --set node-feature-discovery.enabled=true
	rm -f "${values_file}"

	local ds
	ds=$(extract_kata_deploy_ds)

	echo "${ds}" | grep -q "podAntiAffinity:"
	echo "${ds}" | grep -q "gpu-operator"
	echo "${ds}" | grep -q "platform-team"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
}

@test "Helm template: NFD merge preserves matchFields in nodeSelectorTerms" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchFields:
            - key: metadata.name
              operator: In
              values:
                - worker-node-1
EOF

	render_chart -f "${values_file}" --set node-feature-discovery.enabled=true
	rm -f "${values_file}"

	local ds
	ds=$(extract_kata_deploy_ds)

	echo "${ds}" | grep -q "matchFields:"
	echo "${ds}" | grep -q "metadata.name"
	echo "${ds}" | grep -q "worker-node-1"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
}

@test "Helm template: NFD merge cross-products multiple user OR terms" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - platform-team
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - gpu-team
EOF

	render_chart -f "${values_file}" --set node-feature-discovery.enabled=true
	rm -f "${values_file}"

	local ds term_count
	ds=$(extract_kata_deploy_ds)
	term_count=$(count_required_node_selector_terms "${ds}")

	[[ "${term_count}" -eq 12 ]]
	echo "${ds}" | grep -q "platform-team"
	echo "${ds}" | grep -q "gpu-team"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
}

@test "Helm template: NFD merge omits empty matchFields" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - platform-team
EOF

	render_chart -f "${values_file}" --set node-feature-discovery.enabled=true
	rm -f "${values_file}"

	local ds
	ds=$(extract_kata_deploy_ds)

	echo "${ds}" | grep -q "node.cloud/reserved"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
	! echo "${ds}" | grep -q 'matchFields: \[\]'
}

@test "Helm template: NFD merge preserves preferredDuringSchedulingIgnoredDuringExecution" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - platform-team
    preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 100
        preference:
          matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - preferred-team
EOF

	render_chart -f "${values_file}" --set node-feature-discovery.enabled=true
	rm -f "${values_file}"

	local ds
	ds=$(extract_kata_deploy_ds)

	echo "${ds}" | grep -q "preferredDuringSchedulingIgnoredDuringExecution:"
	echo "${ds}" | grep -q "preferred-team"
	echo "${ds}" | grep -q "weight: 100"
	echo "${ds}" | grep -q "platform-team"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
}

@test "Helm template: NFD required applied when user has no required terms" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 50
        preference:
          matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - preferred-team
EOF

	render_chart -f "${values_file}" --set node-feature-discovery.enabled=true
	rm -f "${values_file}"

	local ds term_count
	ds=$(extract_kata_deploy_ds)
	term_count=$(count_required_node_selector_terms "${ds}")

	[[ "${term_count}" -eq 6 ]]
	echo "${ds}" | grep -q "preferredDuringSchedulingIgnoredDuringExecution:"
	echo "${ds}" | grep -q "preferred-team"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
}

# =============================================================================
# Job mode: per-node Job pod-template rendering (deploymentMode: job)
# =============================================================================

@test "Helm template (job mode): per-node Jobs are rendered with default labels" {
	render_job_templates

	local install cleanup
	install=$(extract_pernode_job install)
	cleanup=$(extract_pernode_job cleanup)

	[[ -n "${install}" ]]
	[[ -n "${cleanup}" ]]
	echo "${install}" | grep -q "kata-deploy/stage: install"
	echo "${cleanup}" | grep -q "kata-deploy/stage: cleanup"
	echo "${install}" | grep -A5 "template:" | grep -A4 "labels:" | grep -q "app.kubernetes.io/name: kata-deploy"
	# Affinity is DaemonSet-only; per-node Jobs are pinned via nodeName.
	! echo "${install}" | grep -q "affinity:"
}

@test "Helm template (job mode): podLabels are applied to per-node Job pod templates" {
	render_job_templates --set podLabels.team=platform

	local install cleanup
	install=$(extract_pernode_job install)
	cleanup=$(extract_pernode_job cleanup)

	echo "${install}" | grep -A5 "template:" | grep -A4 "labels:" | grep -q "team: platform"
	echo "${install}" | grep -A5 "template:" | grep -A4 "labels:" | grep -q "app.kubernetes.io/name: kata-deploy"
	echo "${cleanup}" | grep -A5 "template:" | grep -A4 "labels:" | grep -q "team: platform"
}

@test "Helm template (job mode): podAnnotations are applied to per-node Job pod templates" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
podAnnotations:
  example.com/owner: platform-team
  prometheus.io/scrape: "false"
EOF

	render_job_templates -f "${values_file}"
	rm -f "${values_file}"

	local install cleanup
	install=$(extract_pernode_job install)
	cleanup=$(extract_pernode_job cleanup)

	echo "${install}" | grep -A10 "template:" | grep -A5 "metadata:" | grep -q "annotations:"
	echo "${install}" | grep -q "example.com/owner: platform-team"
	echo "${install}" | grep -q 'prometheus.io/scrape: "false"'
	echo "${cleanup}" | grep -q "example.com/owner: platform-team"
}

@test "Helm template (job mode): node affinity is compiled away, not copied into per-node Jobs" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - platform-team
EOF

	render_job_templates -f "${values_file}"

	local install
	install=$(extract_pernode_job install)

	# The per-node Jobs are pinned by nodeName, so affinity would be a no-op
	# there; the rule is enforced by the dispatcher's node query instead.
	[[ -n "${install}" ]]
	! echo "${install}" | grep -q "affinity:"
	! echo "${install}" | grep -q "node.cloud/reserved"

	local selectors
	selectors=$(dispatcher_selectors install -f "${values_file}")
	rm -f "${values_file}"

	[[ "${selectors}" == 'node.cloud/reserved in (platform-team)' ]]
}

# =============================================================================
# Job-mode node selection (nodeSelector / affinity.nodeAffinity)
# =============================================================================

@test "Helm template (job mode): default selects every node and lets taints decide" {
	local selectors
	selectors=$(dispatcher_selectors install)

	# No selector at all: the dispatcher lists every node and then drops the
	# ones whose taints the install does not tolerate, which is what keeps Kata
	# off control-plane nodes without any label filtering.
	[[ -z "${selectors}" ]]
}

@test "Helm template (job mode): nodeSelector is compiled into the node query" {
	local selectors
	selectors=$(dispatcher_selectors install --set nodeSelector.kata-containers=enabled)

	[[ "${selectors}" == 'kata-containers=enabled' ]]
}

@test "Helm template (job mode): control-plane nodes can be targeted by label" {
	local selectors
	selectors=$(dispatcher_selectors install \
		--set 'nodeSelector.node-role\.kubernetes\.io/control-plane=')

	# The label is present with an empty value, so "key=" is the correct query.
	[[ "${selectors}" == 'node-role.kubernetes.io/control-plane=' ]]
}

@test "Helm template (job mode): each nodeSelectorTerm becomes one query (OR)" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: kubernetes.io/os
              operator: In
              values:
                - linux
            - key: nvidia.com/gpu.present
              operator: In
              values:
                - "true"
        - matchExpressions:
            - key: tee
              operator: Exists
EOF

	local selectors
	selectors=$(dispatcher_selectors install -f "${values_file}")
	rm -f "${values_file}"

	[[ "$(echo "${selectors}" | wc -l)" -eq 2 ]]
	echo "${selectors}" | grep -qx 'kubernetes.io/os in (linux),nvidia.com/gpu.present in (true)'
	echo "${selectors}" | grep -qx 'tee'
}

@test "Helm template (job mode): preferred node affinity is ignored, not rejected" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
affinity:
  nodeAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 10
        preference:
          matchExpressions:
            - key: node.cloud/fast
              operator: Exists
EOF

	local selectors
	selectors=$(dispatcher_selectors install -f "${values_file}")
	rm -f "${values_file}"

	# A preference never restricts where a DaemonSet may land, so honouring only
	# the required terms keeps both modes targeting the same nodes.
	[[ -z "${selectors}" ]]
}

@test "Helm template (job mode): NFD virtualization requirements gate node selection" {
	local selectors
	selectors=$(dispatcher_selectors install --set 'node-feature-discovery.enabled=true')

	# One query per NFD virtualization term, mirroring the nodeAffinity the
	# DaemonSet is given, so job mode cannot install on a node the DaemonSet
	# would have refused.
	[[ "$(echo "${selectors}" | wc -l)" -eq 6 ]]
	echo "${selectors}" | grep -qx 'feature.node.kubernetes.io/cpu-cpuid.VMX in (true),kubernetes.io/arch in (amd64)'
	echo "${selectors}" | grep -qx 'feature.node.kubernetes.io/cpu-cpuid.SVM in (true),kubernetes.io/arch in (amd64)'
}

@test "Helm template (job mode): tolerations reach the per-node Jobs the dispatcher reads" {
	render_job_templates \
		--set 'tolerations[0].key=node-role.kubernetes.io/control-plane' \
		--set 'tolerations[0].operator=Exists' \
		--set 'tolerations[0].effect=NoSchedule'

	local install cleanup
	install=$(extract_pernode_job install)
	cleanup=$(extract_pernode_job cleanup)

	# Load-bearing: the dispatcher decides which tainted nodes are eligible by
	# reading the tolerations out of this very template, so dropping them here
	# would silently skip every tainted node. Asserted as whole entries, since a
	# key on one line says nothing about the operator on another.
	echo "${install}" | tolerations_of |
		grep "key: node-role.kubernetes.io/control-plane" | grep -q "operator: Exists"

	# Cleanup tolerates everything instead: a taint added after the install must
	# not leave a node with Kata on it that no uninstall can reach. A key or an
	# effect on that entry would narrow it back down.
	[[ "$(echo "${cleanup}" | tolerations_of)" == "operator: Exists" ]]
}

@test "Helm template (job mode): install Jobs tolerate what a DaemonSet pod tolerates" {
	# The DaemonSet controller adds these to its own pods, and job mode installs on
	# the same nodes. not-ready matters most: the install restarts the CRI runtime,
	# which takes the node NotReady long enough for the taint manager to evict a pod
	# that does not tolerate it.
	render_job_templates

	local install taint
	install=$(extract_pernode_job install)

	for taint in not-ready unreachable disk-pressure memory-pressure pid-pressure unschedulable; do
		echo "${install}" | grep -q "key: node.kubernetes.io/${taint}"
	done
}

@test "Helm template (job mode): uninstall targets labeled nodes and ignores taints" {
	local rendered
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set nodeSelector.kata-containers=enabled \
		--show-only templates/kata-deploy-cleanup-job.yaml)

	# Cleanup must reach every node the install labeled, so it is neither
	# narrowed by the top-level nodeSelector nor blocked by taints added since.
	echo "${rendered}" | grep -q -- '--node-selector=katacontainers.io/kata-runtime'
	refute_match "${rendered}" 'kata-containers=enabled'
	echo "${rendered}" | grep -q -- '--ignore-node-taints'
}

@test "Helm template (job mode): job.nodes wins over the compiled selection" {
	local rendered
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set nodeSelector.kata-containers=enabled \
		--set 'job.nodes[0]=worker-1' \
		--set 'job.nodes[1]=worker-2' \
		--show-only templates/kata-deploy-install-job.yaml)

	echo "${rendered}" | grep -q -- '--nodes=worker-1,worker-2'
	refute_match "${rendered}" '--node-selector='

	# Naming a node is an admission override, so its Job tolerates every taint. A
	# key or an effect on that entry would narrow it back down.
	render_job_templates --set 'job.nodes[0]=worker-1'
	local install
	install=$(extract_pernode_job install)
	[[ "$(echo "${install}" | tolerations_of)" == "operator: Exists" ]]
}

@test "Helm template (job mode): install waits for nodes to become eligible" {
	local rendered
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--show-only templates/kata-deploy-install-job.yaml)

	# The dispatcher runs once, but eligibility arrives late: the labels the
	# selection matches are written by NFD, which starts with this very release.
	# Resolving nodes off the first snapshot would install nowhere and exit 0.
	echo "${rendered}" | grep -q -- '--wait-for-nodes-secs=120'
	echo "${rendered}" | grep -q -- '--node-settle-secs=15'
	echo "${rendered}" | grep -q -- '--cleanup-job-template=/etc/kata-job/cleanup-job.yaml'

	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set job.waitForNodesSeconds=0 \
		--show-only templates/kata-deploy-install-job.yaml)

	echo "${rendered}" | grep -q -- '--wait-for-nodes-secs=0'
}

@test "Helm template (job mode): named nodes and uninstall never wait" {
	local install cleanup
	install=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set 'job.nodes[0]=worker-1' \
		--show-only templates/kata-deploy-install-job.yaml)
	cleanup=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--show-only templates/kata-deploy-cleanup-job.yaml)

	# Named nodes need no discovery, and uninstall must not stall for two
	# minutes on a release that never labeled a node in the first place.
	refute_match "${install}" '--wait-for-nodes-secs'
	refute_match "${cleanup}" '--wait-for-nodes-secs'
}

# =============================================================================
# Node-selection validation (one source of truth)
# =============================================================================

@test "Helm template: nodeSelector and nodeAffinity are ANDed, as Kubernetes does" {
	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
nodeSelector:
  kata-containers: "enabled"
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: kubernetes.io/os
              operator: In
              values:
                - linux
        - matchExpressions:
            - key: tee
              operator: Exists
EOF

	# Daemonset mode hands both to the scheduler untouched and lets Kubernetes
	# AND them, which is the behaviour job mode has to reproduce.
	render_chart -f "${values_file}"
	local ds
	ds=$(extract_kata_deploy_ds)
	echo "${ds}" | grep -q "kata-containers: enabled"
	echo "${ds}" | grep -q "nodeAffinity:"

	# Job mode folds the nodeSelector equalities into EVERY term, which is the
	# same node set by distribution: eq AND (t1 OR t2) == (eq AND t1) OR (eq AND t2).
	local selectors
	selectors=$(dispatcher_selectors install -f "${values_file}")
	rm -f "${values_file}"

	[[ "$(echo "${selectors}" | wc -l)" -eq 2 ]]
	echo "${selectors}" | grep -qx 'kata-containers=enabled,kubernetes.io/os in (linux)'
	echo "${selectors}" | grep -qx 'kata-containers=enabled,tee'
}

@test "Helm template: nodeSelector combined with podAntiAffinity is allowed" {
	run helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=daemonset \
		--set nodeSelector.kata-containers=enabled \
		--set 'affinity.podAntiAffinity.requiredDuringSchedulingIgnoredDuringExecution[0].topologyKey=kubernetes.io/hostname'

	[ "${status}" -eq 0 ]
}

@test "Helm template: removed job-mode selection keys fail with a migration hint" {
	run helm template kata-deploy "${CHART_PATH}" --set deploymentMode=job \
		--set job.nodeSelector.kata-containers=enabled
	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q "job.nodeSelector has been removed"

	run helm template kata-deploy "${CHART_PATH}" --set deploymentMode=job \
		--set 'job.nodeSelectorExpressions[0].key=kata' \
		--set 'job.nodeSelectorExpressions[0].operator=Exists'
	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q "job.nodeSelectorExpressions has been removed"

	run helm template kata-deploy "${CHART_PATH}" --set deploymentMode=job \
		--set 'job.cleanup.nodeSelectorExpressions[0].key=kata' \
		--set 'job.cleanup.nodeSelectorExpressions[0].operator=Exists'
	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q "renamed to job.cleanup.nodeAffinity"
}

@test "Helm template (job mode): rules a node query cannot express are rejected" {
	run helm template kata-deploy "${CHART_PATH}" --set deploymentMode=job \
		--set 'affinity.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms[0].matchExpressions[0].key=cpus' \
		--set 'affinity.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms[0].matchExpressions[0].operator=Gt'
	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q 'unsupported operator "Gt"'

	run helm template kata-deploy "${CHART_PATH}" --set deploymentMode=job \
		--set 'affinity.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms[0].matchFields[0].key=metadata.name' \
		--set 'affinity.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms[0].matchFields[0].operator=In'
	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q "matchFields cannot be used"
}

@test "Helm template (job mode): empty required affinity cannot invert to every node" {
	run helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set-json 'affinity.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms=[]'
	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q 'nodeSelectorTerms must not be empty'

	run helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set-json 'affinity.nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms=[{}]'
	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q 'an empty nodeSelectorTerm matches no nodes'
}

@test "Helm template (job mode): the dispatcher can be confined to trusted nodes" {
	local args=(
		--set 'job.dispatcherNodeSelector.node-role\.kubernetes\.io/control-plane='
		--set 'job.dispatcherTolerations[0].key=node-role.kubernetes.io/control-plane'
		--set 'job.dispatcherTolerations[0].operator=Exists'
		--set 'job.dispatcherTolerations[0].effect=NoSchedule'
	)

	# The dispatcher's token is the one that reaches every node in the cluster, so
	# an operator must be able to keep it off the nodes Kata runs on: root on the
	# node it lands on can read that token.
	local stage rendered
	for stage in install cleanup; do
		rendered=$(helm template kata-deploy "${CHART_PATH}" \
			--set deploymentMode=job \
			"${args[@]}" \
			--show-only "templates/kata-deploy-${stage}-job.yaml")
		echo "${rendered}" | grep -q 'node-role.kubernetes.io/control-plane: ""'
		echo "${rendered}" | grep -q 'key: node-role.kubernetes.io/control-plane'
	done

	# Where the dispatcher may run says nothing about where Kata is installed: the
	# per-node Jobs must not inherit its placement, or pinning the dispatcher to
	# the control plane would quietly stop installing on the workers.
	local jobs
	jobs=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		"${args[@]}" \
		--show-only templates/kata-deploy-job-templates.yaml)
	# Spelled out rather than `! ... grep`, whose return value bash leaves out of
	# set -e: as anything but the last line of a test, it cannot fail it.
	if echo "${jobs}" | grep -q 'node-role.kubernetes.io/control-plane'; then
		echo "per-node Jobs inherited the dispatcher's placement" >&2
		return 1
	fi
}

@test "Helm template (job mode): dispatcher tolerations default to the top-level ones" {
	local rendered
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set 'tolerations[0].operator=Exists' \
		--show-only templates/kata-deploy-install-job.yaml)

	# Without this fallback a cluster whose every node is tainted - a single-node
	# cluster, say - would select nodes it has nowhere to dispatch from.
	echo "${rendered}" | grep -q 'tolerations:'
	echo "${rendered}" | grep -q 'operator: Exists'
}

@test "Helm template (job mode): a per-node Job cannot run forever" {
	# The dispatcher waits for every node it dispatched to, so one host wedged on a
	# restart that never returns would hold up the whole rollout.
	render_job_templates

	local install
	install=$(extract_pernode_job install)
	echo "${install}" | grep -q "activeDeadlineSeconds: 3600"
}

@test "Helm template (job mode): a Job TTL the dispatcher could not observe is refused" {
	# A Job deleted before the next poll leaves its node with no result, and that
	# counts as a failure: an install that worked, reported as broken.
	run helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set job.ttlSecondsAfterFinished=30
	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q "too short for the dispatcher to observe"
}
