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

	local ds
	ds=$(extract_kata_deploy_ds)

	echo "${ds}" | grep -q "node.cloud/reserved"
	echo "${ds}" | grep -q "platform-team"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.VMX"
	echo "${ds}" | grep -q "feature.node.kubernetes.io/cpu-cpuid.SVM"
}
