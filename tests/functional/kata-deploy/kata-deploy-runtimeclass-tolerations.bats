#!/usr/bin/env bats
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for RuntimeClass tolerations (runtimeClasses.tolerations
# and shims.<shim>.runtimeClass.tolerations). No cluster required.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"

GLOBAL_TOLERATION='[{"key": "global", "operator": "Exists", "effect": "NoSchedule"}]'
SHIM_TOLERATION='[{"key": "per-shim", "operator": "Equal", "value": "gpu", "effect": "NoSchedule"}]'

setup_file() {
	ensure_yq
}

setup() {
	RENDERED="$(mktemp)"
	VALUES_FILE="$(mktemp)"
}

teardown() {
	rm -f "${RENDERED}" "${VALUES_FILE}"
}

render_chart() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		"$@" > "${RENDERED}"
}

# Run a yq expression over every rendered RuntimeClass, without document separators.
# Helm emits empty documents too, so drop the blank lines.
query_runtime_classes() {
	yq -r --no-doc "select(.kind == \"RuntimeClass\") | ${1}" "${RENDERED}" | sed '/^$/d'
}

# Names of all rendered RuntimeClasses, one per line.
runtime_classes() {
	query_runtime_classes '.metadata.name'
}

# Toleration keys of one RuntimeClass, comma-separated and in order.
toleration_keys() {
	query_runtime_classes "select(.metadata.name == \"${1}\") | [(.scheduling.tolerations // [])[].key] | join(\",\")"
}

# A custom runtime whose RuntimeClass manifest declares a toleration of its own.
write_custom_runtime_values() {
	cat > "${VALUES_FILE}" <<EOF
customRuntimes:
  enabled: true
  runtimes:
    with-own:
      baseConfig: "qemu"
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: kata-with-own
        handler: kata-with-own
        scheduling:
          tolerations:
            - key: manifest
              operator: Exists
              effect: NoSchedule
    without-scheduling:
      baseConfig: "qemu"
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: kata-without-scheduling
        handler: kata-without-scheduling
EOF
}

@test "Helm template: no RuntimeClass has tolerations by default" {
	render_chart

	[[ -n "$(runtime_classes)" ]]
	local with_tolerations
	with_tolerations=$(query_runtime_classes 'select(.scheduling.tolerations != null) | .metadata.name')
	[[ -z "${with_tolerations}" ]]
}

@test "Helm template: global tolerations reach every generated RuntimeClass" {
	render_chart \
		--set-json "runtimeClasses.tolerations=${GLOBAL_TOLERATION}" \
		--set runtimeClasses.createDefault=true \
		--set debug=true \
		--set devkit=true

	local classes class
	classes=$(runtime_classes)

	# Make sure the variants are really part of the render.
	grep -qx "kata" <<< "${classes}"
	grep -qx "kata-qemu-debug" <<< "${classes}"
	grep -qx "kata-qemu-devkit" <<< "${classes}"

	for class in ${classes}; do
		echo "checking ${class}"
		[[ "$(toleration_keys "${class}")" == "global" ]]
	done
}

@test "Helm template: per-shim tolerations are appended to the global ones" {
	render_chart \
		--set-json "runtimeClasses.tolerations=${GLOBAL_TOLERATION}" \
		--set-json "shims.qemu.runtimeClass.tolerations=${SHIM_TOLERATION}"

	[[ "$(toleration_keys kata-qemu)" == "global,per-shim" ]]
	# Other shims only get the global list.
	[[ "$(toleration_keys kata-clh)" == "global" ]]
}

@test "Helm template: per-shim tolerations work without global ones" {
	render_chart \
		--set-json "shims.qemu.runtimeClass.tolerations=${SHIM_TOLERATION}"

	[[ "$(toleration_keys kata-qemu)" == "per-shim" ]]
	[[ -z "$(toleration_keys kata-clh)" ]]
}

@test "Helm template: toleration fields are rendered unchanged" {
	render_chart \
		--set-json "shims.qemu.runtimeClass.tolerations=${SHIM_TOLERATION}"

	local toleration
	toleration=$(query_runtime_classes 'select(.metadata.name == "kata-qemu") | .scheduling.tolerations[0] | to_json(0)')
	[[ "${toleration}" == '{"effect":"NoSchedule","key":"per-shim","operator":"Equal","value":"gpu"}' ]]
}

@test "Helm template: tolerations keep the RuntimeClass nodeSelector" {
	render_chart \
		--set-json "runtimeClasses.tolerations=${GLOBAL_TOLERATION}"

	local selector
	selector=$(query_runtime_classes 'select(.metadata.name == "kata-qemu") | .scheduling.nodeSelector["katacontainers.io/kata-runtime"]')
	[[ "${selector}" == "true" ]]
}

@test "Helm template: custom runtimes put global tolerations before their own" {
	write_custom_runtime_values
	render_chart -f "${VALUES_FILE}" \
		--set-json "runtimeClasses.tolerations=${GLOBAL_TOLERATION}"

	[[ "$(toleration_keys kata-with-own)" == "global,manifest" ]]
	[[ "$(toleration_keys kata-without-scheduling)" == "global" ]]
}

@test "Helm template: custom runtimes keep their own tolerations without global ones" {
	write_custom_runtime_values
	render_chart -f "${VALUES_FILE}"

	[[ "$(toleration_keys kata-with-own)" == "manifest" ]]
	[[ -z "$(toleration_keys kata-without-scheduling)" ]]
}
