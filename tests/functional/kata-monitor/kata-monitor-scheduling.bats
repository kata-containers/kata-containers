#!/usr/bin/env bats
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for kata-monitor DaemonSet scheduling options (affinity).
# No cluster required.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/../kata-deploy/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"
RENDERED="/tmp/kata-monitor-scheduling-rendered.yaml"

render_chart() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		"$@" > "${RENDERED}"
}

# Extract the kata-monitor DaemonSet manifest.
extract_kata_monitor_ds() {
	awk '
		/^kind: DaemonSet$/ { buf = $0 "\n"; in_ds = 1; has_name = 0; next }
		in_ds {
			buf = buf $0 "\n"
			if ($0 ~ /^  name: kata-monitor$/) { has_name = 1 }
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

@test "Helm template: kata-monitor default values have no affinity" {
	render_chart --set monitor.enabled=true

	local ds
	ds=$(extract_kata_monitor_ds)

	[[ -n "${ds}" ]]
	echo "${ds}" | grep -q "name: kata-monitor"
	! echo "${ds}" | grep -q "affinity:"
}

@test "Helm template: kata-monitor user affinity is applied to pod spec" {
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

	render_chart -f "${values_file}" --set monitor.enabled=true
	rm -f "${values_file}"

	local ds
	ds=$(extract_kata_monitor_ds)

	echo "${ds}" | grep -q "affinity:"
	echo "${ds}" | grep -q "node.cloud/reserved"
	echo "${ds}" | grep -q "platform-team"
	echo "${ds}" | grep -q "podAntiAffinity:"
	echo "${ds}" | grep -q "gpu-operator"
}

@test "Helm template: kata-monitor NFD enabled merges virtualization nodeAffinity" {
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

	render_chart -f "${values_file}" --set monitor.enabled=true --set node-feature-discovery.enabled=true
	rm -f "${values_file}"

	local ds term_count
	ds=$(extract_kata_monitor_ds)
	term_count=$(count_required_node_selector_terms "${ds}")

	[[ "${term_count}" -eq 6 ]]
	echo "${ds}" | grep -q "node.cloud/reserved"
	echo "${ds}" | grep -q "platform-team"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.SVM"
}
