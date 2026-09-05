#!/usr/bin/env bats
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for the TEE key advertisement: the NodeFeatureRule that
# publishes a node's SEV-SNP encrypted-state IDs / TDX key slots, and the
# matching overhead.podFixed requests on the confidential RuntimeClasses.
# No cluster required.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"
RENDERED="/tmp/kata-deploy-tee-keys-rendered.yaml"

# The confidential shims are not in the chart defaults - they ship in the TEE
# profile, together with the nydus setup and guest pull they need - so every
# render here goes through it. Without it there is no confidential RuntimeClass
# to carry a key request and these tests would pass by having nothing to check.
render_chart() {
	helm template kata-deploy "${CHART_PATH}" \
		--set image.reference=quay.io/kata-containers/kata-deploy \
		--set image.tag=latest \
		-f "${CHART_PATH}/try-kata-tee.values.yaml" \
		"$@" > "${RENDERED}"
}

# `! grep ...` would be exempt from set -e and could never fail a test.
refute_rendered() {
	if grep -q "$1" "${RENDERED}"; then
		echo "unexpected in rendered chart: $1" >&2
		return 1
	fi
}

@test "Helm template: the TEE rule and the key requests are rendered together" {
	render_chart --set nodeFeatureRules.create=true

	grep -q "kind: NodeFeatureRule" "${RENDERED}"
	grep -q "sev-snp.amd.com/esids" "${RENDERED}"
	grep -q "tdx.intel.com/keys" "${RENDERED}"
}

@test "Helm template: nothing requests a TEE key without the rule advertising it" {
	# A request for an extended resource nothing advertises leaves every pod on
	# that RuntimeClass Pending forever, so these two must never come apart.
	local args
	for args in "--set nodeFeatureRules.create=false" \
		"--set node-feature-discovery.enabled=true --set nodeFeatureRules.create=false" \
		""; do
		# shellcheck disable=SC2086
		render_chart ${args}

		refute_rendered "kind: NodeFeatureRule"
		refute_rendered "sev-snp.amd.com/esids"
		refute_rendered "tdx.intel.com/keys"
	done
}

@test "Helm template: auto follows the NFD shipped with the release" {
	render_chart --set node-feature-discovery.enabled=true

	grep -q "kind: NodeFeatureRule" "${RENDERED}"
	grep -q "sev-snp.amd.com/esids" "${RENDERED}"
}

@test "Helm template: only the confidential RuntimeClasses ask for a key" {
	render_chart --set nodeFeatureRules.create=true

	# Every class carrying a key request names a TEE in its handler: a
	# non-confidential sandbox holding one would waste a slot the hardware
	# has a fixed number of.
	local handler
	while read -r handler; do
		[[ "${handler}" =~ (snp|tdx) ]]
	done < <(awk '
		/^kind: / { kind = $2; handler = "" }
		kind != "RuntimeClass" { next }
		/^handler: / { handler = $2 }
		/(sev-snp\.amd\.com\/esids|tdx\.intel\.com\/keys):/ { print handler }
	' "${RENDERED}")
}

@test "Helm template: an unusable nodeFeatureRules.create is rejected" {
	# Silently falling back to auto would hand back a cluster configured other
	# than as asked for.
	run render_chart --set nodeFeatureRules.create=maybe

	[[ "${status}" -ne 0 ]]
	[[ "${output}" == *"nodeFeatureRules.create must be one of"* ]]
}
