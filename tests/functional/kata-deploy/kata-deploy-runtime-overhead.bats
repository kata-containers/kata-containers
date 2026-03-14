#!/usr/bin/env bats
#
# Copyright (c) 2026 CoreWeave, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# Tests for kata-deploy RuntimeClass pod overhead override
# (values.yaml: shims.<name>.runtimeClass.overhead memory/cpu).
#
# This file currently contains only Helm template rendering tests and can be
# run locally with just Helm installed.
#
# Note: End-to-end (E2E) kata-deploy tests live in separate *.bats files and
# will document any additional required environment variables themselves.
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

setup() {
	ensure_helm

	# Build chart dependencies so this file can be run in isolation (e.g. bats kata-deploy-runtime-overhead.bats
	# or KATA_DEPLOY_TEST_UNION with only this file). Other kata-deploy tests rely on kata-deploy.bats running
	# first (deploy_kata builds deps); we do not, so template tests work when run alone.
	local chart_path
	chart_path="$(get_chart_path)"
	helm repo add node-feature-discovery https://kubernetes-sigs.github.io/node-feature-discovery/charts 2>/dev/null || true
	helm repo update
	helm dependency build "${chart_path}"
}

@test "Helm template: RuntimeClass pod overhead can be overridden via shims.<name>.runtimeClass.overhead" {
	local chart_path
	chart_path="$(get_chart_path)"

	# Use distinct prime values so we verify the override path (qemu default is 160Mi/250m)
	local override_memory="317Mi"
	local override_cpu="137m"

	local values_file
	values_file=$(mktemp)
	cat > "${values_file}" <<EOF
image:
  reference: quay.io/kata-containers/kata-deploy
  tag: latest

shims:
  disableAll: true
  qemu:
    enabled: true
    runtimeClass:
      overhead:
        memory: "${override_memory}"
        cpu: "${override_cpu}"

defaultShim:
  amd64: qemu
  arm64: qemu

runtimeClasses:
  enabled: true
  createDefault: true
EOF

	helm template kata-deploy "${chart_path}" -f "${values_file}" > /tmp/rendered-overhead.yaml
	rm -f "${values_file}"

	# Assert RuntimeClass kata-qemu exists
	grep -q "name: kata-qemu" /tmp/rendered-overhead.yaml
	grep -q "handler: kata-qemu" /tmp/rendered-overhead.yaml

	# Extract the RuntimeClass document for kata-qemu specifically, then check its overhead block
	local kata_qemu_runtimeclass_file
	kata_qemu_runtimeclass_file=$(mktemp)
	awk '
		/name: kata-qemu/ { in_doc=1 }
		in_doc && /^---/ { exit }
		in_doc { print }
	' /tmp/rendered-overhead.yaml > "${kata_qemu_runtimeclass_file}"

	grep -A4 "overhead:" "${kata_qemu_runtimeclass_file}" | grep -q "memory: \"${override_memory}\""
	grep -A4 "overhead:" "${kata_qemu_runtimeclass_file}" | grep -q "cpu: \"${override_cpu}\""
	rm -f "${kata_qemu_runtimeclass_file}"
}
