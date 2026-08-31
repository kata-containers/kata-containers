#!/usr/bin/env bats
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Helm template tests for running several kata-deploy installations side by side
# (env.multiInstallSuffix). No cluster required.
#
# Everything belonging to one installation is named after its suffix, except
# katacontainers.io/kata-runtime: every installation's RuntimeClasses select it, so
# it says "this node can run Kata", not "this installation is here". Each
# installation therefore marks its own nodes as well, and an uninstall gives the
# shared label up only once no other mark is left.

load "${BATS_TEST_DIRNAME}/../../common.bash"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"

CHART_PATH="$(get_chart_path)"

render_dispatcher() {
	local stage="${1}"
	shift
	helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--show-only "templates/kata-deploy-${stage}-job.yaml" \
		"$@"
}

refute_match() {
	local rendered="${1}" pattern="${2}"
	if echo "${rendered}" | grep -q -- "${pattern}"; then
		echo "unexpected in rendered output: ${pattern}" >&2
		return 1
	fi
}

@test "Helm template (job mode): both dispatchers know which install they are" {
	local stage rendered
	for stage in install cleanup; do
		rendered=$(render_dispatcher "${stage}" --set env.multiInstallSuffix=dev)
		echo "${rendered}" | grep -q -- '--multi-install-suffix=dev'
	done
}

@test "Helm template (job mode): a single install needs no suffix to say so" {
	# The dispatcher falls back to the "default" instance, so the flag is only
	# rendered when there is something to say.
	local stage rendered
	for stage in install cleanup; do
		rendered=$(render_dispatcher "${stage}")
		refute_match "${rendered}" '--multi-install-suffix'
	done
}

@test "Helm template: the suffix is persisted under a stable release name" {
	# Every host path, handler, and marker derives from the suffix. Keeping the
	# record under a name that does not derive from it lets a live upgrade reject
	# an in-place suffix change instead of orphaning the old installation.
	local rendered
	rendered=$(helm template my-release "${CHART_PATH}" \
		--set env.multiInstallSuffix=dev \
		--show-only templates/kata-deploy-state.yaml)

	echo "${rendered}" | grep -q 'name: my-release-kata-deploy-state'
	echo "${rendered}" | grep -q 'multiInstallSuffix: "dev"'
	echo "${rendered}" | grep -q 'deploymentMode: "job"'
}

@test "Helm template: an unverifiable pre-state upgrade is refused" {
	# A suffix change makes every old mode-specific resource disappear under the
	# newly derived name. Proceeding would create a second installation and forget
	# the first, so the safe answer is to require the previous identity to be
	# restored or seeded.
	run helm template my-release "${CHART_PATH}" \
		--is-upgrade \
		--set env.multiInstallSuffix=dev \
		--show-only templates/kata-deploy-state.yaml

	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q 'cannot verify the previous kata-deploy identity'
}

@test "Helm template: deployment mode is validated before state is persisted" {
	run helm template my-release "${CHART_PATH}" --set deploymentMode=bogus

	[ "${status}" -ne 0 ]
	echo "${output}" | grep -q 'deploymentMode must be one of daemonset or job'
}

@test "Helm template: the node label the RuntimeClasses select is never suffixed" {
	# Which is why the per-install mark exists: this label cannot tell two
	# installations apart, so removing it cannot be decided from it.
	local rendered
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set env.multiInstallSuffix=dev \
		--show-only templates/runtimeclasses.yaml)

	echo "${rendered}" | grep -q 'katacontainers.io/kata-runtime: "true"'
	refute_match "${rendered}" 'katacontainers.io/kata-runtime-dev'
}

@test "Helm template: an install's RuntimeClasses select its own mark as well" {
	# Without this, removing an install from a node would not stop its own workloads
	# landing there, since the shared label stays while another install holds it.
	local rendered
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set env.multiInstallSuffix=dev \
		--show-only templates/runtimeclasses.yaml)

	echo "${rendered}" | grep -q 'kata-deploy.katacontainers.io/dev: "true"'
}

@test "Helm template: a shim's own nodeSelector cannot weaken the ownership gate" {
	# The gate and the shim's selector end up in one YAML mapping, where a repeated
	# key is resolved in the user's favour, pointing the RuntimeClass at nodes this
	# install does not hold.
	local rendered qemu
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set env.multiInstallSuffix=dev \
		--set 'shims.qemu.runtimeClass.nodeSelector.katacontainers\.io/kata-runtime=false' \
		--set 'shims.qemu.runtimeClass.nodeSelector.kata-deploy\.katacontainers\.io/dev=false' \
		--set 'shims.qemu.runtimeClass.nodeSelector.disktype=ssd' \
		--show-only templates/runtimeclasses.yaml)

	refute_match "${rendered}" 'katacontainers.io/kata-runtime: "false"'
	refute_match "${rendered}" 'kata-deploy.katacontainers.io/dev: "false"'

	qemu=$(echo "${rendered}" | awk '/^handler: kata-qemu-dev$/,/^---$/')
	[ "$(echo "${qemu}" | grep -c 'katacontainers.io/kata-runtime:')" -eq 1 ]
	[ "$(echo "${qemu}" | grep -c 'kata-deploy.katacontainers.io/dev:')" -eq 1 ]
	# Everything else the shim asked for is still there.
	echo "${qemu}" | grep -q 'disktype: "ssd"'
}

@test "Helm template: the default install's RuntimeClasses select its mark too" {
	# "Default" is one installation like any other. Without its mark in the selector,
	# removing it while another release holds the shared label would keep default
	# workloads landing on a node whose default runtime is going away.
	local rendered
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--show-only templates/runtimeclasses.yaml)

	echo "${rendered}" | grep -q 'katacontainers.io/kata-runtime: "true"'
	echo "${rendered}" | grep -q 'kata-deploy.katacontainers.io/default: "true"'
}

@test "Helm template: custom RuntimeClasses are gated on the mark too" {
	local rendered
	rendered=$(helm template kata-deploy "${CHART_PATH}" \
		--set deploymentMode=job \
		--set env.multiInstallSuffix=dev \
		--set customRuntimes.enabled=true \
		--set customRuntimes.runtimes.mine.baseConfig=qemu \
		--set-string customRuntimes.runtimes.mine.runtimeClass='kind: RuntimeClass
apiVersion: node.k8s.io/v1
metadata:
  name: mine
handler: mine
scheduling:
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
    kata-deploy.katacontainers.io/dev: "false"
' \
		--show-only templates/custom-runtimes.yaml)

	echo "${rendered}" | grep -q 'kata-deploy.katacontainers.io/dev: "true"'
	refute_match "${rendered}" 'kata-deploy.katacontainers.io/dev: "false"'
	echo "${rendered}" | grep -q 'katacontainers.io/kata-runtime: "true"'
}
