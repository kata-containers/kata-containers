#!/usr/bin/env bats
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for kata-deploy DaemonSet scheduling options
# (podLabels, podAnnotations, affinity). No cluster required.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"
RENDERED="/tmp/kata-deploy-scheduling-rendered.yaml"

render_chart() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		"$@" > "${RENDERED}"
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
