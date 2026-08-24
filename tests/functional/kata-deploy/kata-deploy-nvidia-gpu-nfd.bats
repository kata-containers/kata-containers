#!/usr/bin/env bats
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for the NVIDIA GPU NodeFeatureRule: the labels it asks
# node-feature-discovery to publish, and the RuntimeClasses that select on them.
# No cluster required.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"
RENDERED="/tmp/kata-deploy-nvidia-gpu-nfd-rendered.yaml"

render_chart() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		"$@" > "${RENDERED}"
}

# `! grep ...` would be exempt from set -e and could never fail a test.
refute_rendered() {
	if grep -q "$1" "${RENDERED}"; then
		echo "unexpected in rendered chart: $1" >&2
		return 1
	fi
}

# The rendered YAML of the one object with this metadata.name, so a grep cannot
# match a neighbouring object instead.
document_of() {
	awk -v name="$1" 'BEGIN { RS = "\n---\n" } index($0, "\n  name: " name "\n") { print }' \
		"${RENDERED}"
}

# The node label keys a NodeFeatureRule document on stdin publishes, one per
# line. Only the ones under spec: the object's own metadata.labels are chart
# bookkeeping, not something NFD puts on a node.
published_label_keys() {
	awk '
		!started && /^spec:/ { started = 1; next }
		!started { next }
		/^[[:space:]]*labels:[[:space:]]*$/ {
			match($0, /^[[:space:]]*/)
			indent = RLENGTH
			inside = 1
			next
		}
		inside {
			match($0, /^[[:space:]]*/)
			if (RLENGTH <= indent) { inside = 0; next }
			key = $1
			sub(/:$/, "", key)
			gsub(/"/, "", key)
			print key
		}
	'
}

# The nodeSelector of a RuntimeClass document on stdin, as key=value lines.
node_selector_of() {
	awk '
		/^[[:space:]]*nodeSelector:[[:space:]]*$/ {
			match($0, /^[[:space:]]*/)
			indent = RLENGTH
			inside = 1
			next
		}
		inside {
			match($0, /^[[:space:]]*/)
			if (RLENGTH <= indent) { inside = 0; next }
			key = $1
			sub(/:$/, "", key)
			gsub(/"/, "", key)
			value = $2
			gsub(/"/, "", value)
			print key "=" value
		}
	'
}

@test "Helm template: the NVIDIA GPU rule publishes presence, family and CC capability" {
	render_chart --set nodeFeatureRules.create=true

	local rule
	rule="$(document_of kata-nvidia-gpu)"
	[[ -n "${rule}" ]]

	echo "${rule}" | grep -q "kind: NodeFeatureRule"
	echo "${rule}" | grep -q "nvidia.feature.node.kubernetes.io/gpu\": \"true\""
	echo "${rule}" | grep -q "nvidia.feature.node.kubernetes.io/gpu.family"
	echo "${rule}" | grep -q "nvidia.feature.node.kubernetes.io/cc.capable"

	# Presence is matched by PCI class, not by an ever-growing device list, so
	# a GPU newer than the chart still gets the label.
	echo "${rule}" | grep -q 'class: { op: In, value: \["0300", "0302"\] }'
}

@test "Helm template: no NVIDIA GPU rule without NFD to act on it" {
	local args
	for args in "--set nodeFeatureRules.create=false" \
		"--set node-feature-discovery.enabled=true --set nodeFeatureRules.create=false" \
		""; do
		# shellcheck disable=SC2086
		render_chart ${args}

		refute_rendered "name: kata-nvidia-gpu"
		refute_rendered "nvidia.feature.node.kubernetes.io/gpu.family"
	done
}

@test "Helm template: the GPU labels stay in a namespace NFD publishes by default" {
	# NFD drops labels outside feature.node.kubernetes.io and its subdomains
	# unless nfd-master runs with -extra-label-ns, which this chart cannot set
	# for an NFD installation it does not own. A label in, say, nvidia.com would
	# render fine here and then never reach a node.
	render_chart --set nodeFeatureRules.create=true

	local key
	while read -r key; do
		[[ "${key}" == */* ]]
		[[ "${key%%/*}" == "feature.node.kubernetes.io" ||
			"${key%%/*}" == *".feature.node.kubernetes.io" ]]
	done < <(document_of kata-nvidia-gpu | published_label_keys)
}

@test "Helm template: the family device IDs come from values" {
	# A GPU released after the chart should need a value override, not a new
	# chart, so the device lists must be data rather than baked into the rule.
	render_chart --set nodeFeatureRules.create=true \
		--set nodeFeatureRules.nvidiaGpu.families.hopper=null \
		--set nodeFeatureRules.nvidiaGpu.families.blackwell=null \
		--set nodeFeatureRules.nvidiaGpu.families.imaginary[0]=beef \
		--set nodeFeatureRules.nvidiaGpu.ccCapableFamilies={imaginary}

	local rule
	rule="$(document_of kata-nvidia-gpu)"

	echo "${rule}" | grep -q 'nvidia.gpu.family.imaginary'
	echo "${rule}" | grep -q '"beef"'
	refute_rendered "nvidia.gpu.family.hopper"
}

@test "Helm template: every CC-capable family has device IDs to match on" {
	# A family named in ccCapableFamilies but absent from families sets no var,
	# leaving that branch of the CC rule dead: the label would quietly never
	# appear on a node holding exactly that GPU.
	render_chart --set nodeFeatureRules.create=true

	local rule referenced family
	rule="$(document_of kata-nvidia-gpu)"

	while read -r referenced; do
		family="${referenced#kata.nvidia.gpu.family.}"
		echo "${rule}" | grep -q "name: \"nvidia.gpu.family.${family}\""
	done < <(echo "${rule}" |
		grep -o 'kata\.nvidia\.gpu\.family\.[a-z0-9-]*' | sort -u)
}

@test "Helm template: the plain GPU RuntimeClasses select the chart's own GPU label" {
	# These used to select the GPU Operator's nvidia.com/cc.ready.state, which
	# made a plain GPU sandbox wait for a CC manager it has no use for.
	render_chart --set nodeFeatureRules.create=true

	local shim selectors
	for shim in kata-qemu-nvidia-gpu kata-qemu-nvidia-gpu-runtime-rs; do
		selectors="$(document_of "${shim}" | node_selector_of)"

		echo "${selectors}" | grep -q "^nvidia.feature.node.kubernetes.io/gpu=true$"
		if echo "${selectors}" | grep -q "cc.ready.state"; then
			echo "${shim} still waits on the GPU Operator: ${selectors}" >&2
			return 1
		fi
	done
}

@test "Helm template: the confidential GPU RuntimeClasses still require a GPU in CC mode" {
	# cc.capable says the GPU could run confidentially; only cc.ready.state says
	# it has been put in that mode. Selecting on capability alone would place a
	# confidential sandbox on a GPU running in the clear.
	render_chart --set nodeFeatureRules.create=true

	local shim selectors
	for shim in kata-qemu-nvidia-gpu-snp kata-qemu-nvidia-gpu-tdx; do
		selectors="$(document_of "${shim}" | node_selector_of)"

		echo "${selectors}" | grep -q "^nvidia.com/cc.ready.state=true$"
	done
}

@test "Helm template: a second install gets its own NVIDIA GPU rule" {
	# NodeFeatureRules are cluster scoped, so two installs sharing one name
	# would fight over the object.
	render_chart --set nodeFeatureRules.create=true \
		--set env.multiInstallSuffix=second

	grep -q "name: kata-nvidia-gpu-second" "${RENDERED}"
	refute_rendered "name: kata-nvidia-gpu$"
}
