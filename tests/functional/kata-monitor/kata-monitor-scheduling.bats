#!/usr/bin/env bats
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for kata-monitor DaemonSet scheduling options (affinity).
# No cluster required.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/../kata-deploy/lib/helm-deploy.bash"

RENDERED="/tmp/kata-monitor-scheduling-rendered.yaml"

# Extract the kata-monitor DaemonSet manifest.
extract_kata_monitor_ds() {
	extract_daemonset "kata-monitor"
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
