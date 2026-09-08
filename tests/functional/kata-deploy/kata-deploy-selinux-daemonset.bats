#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# The installer's SELinux confinement in daemonset mode, on a real enforcing node.
#
# This mode runs every stage in one container, which therefore needs the union of
# their domains, kata_deploy_t. Job mode passing says nothing about that union: a
# permission one stage's domain grants can still be missing from it. Unconfined,
# the install runs as container_t and dies in AVC denials (#13751).
#
# Required environment variables:
#   DOCKER_REGISTRY - Container registry for kata-deploy image
#   DOCKER_REPO     - Repository name for kata-deploy image
#   DOCKER_TAG      - Image tag to test
#   KATA_HYPERVISOR - Hypervisor to test (qemu, clh, etc.)
#   KUBERNETES      - K8s distribution

load "${BATS_TEST_DIRNAME}/../../common.bash"
repo_root_dir="${BATS_TEST_DIRNAME}/../../../"
load "${repo_root_dir}/tests/gha-run-k8s-common.sh"

source "${BATS_TEST_DIRNAME}/lib/helm-deploy.bash"
source "${BATS_TEST_DIRNAME}/lib/selinux.bash"

# Matched on the binary behind /proc/PID/exe, not the command line, which would
# also match the shell doing the matching.
kata_deploy_process_contexts() {
	run_on_host 'for p in /host/proc/[0-9]*; do case $(readlink $p/exe 2>/dev/null) in */kata-deploy) cat $p/attr/current 2>/dev/null; echo;; esac; done'
}

setup_file() {
	skip_unless_enforcing_node
	ensure_helm
	assert_chart_supports_selinux

	# There is no dispatcher here to report a failure: a denied install just
	# crashloops until helm gives up, so do not give it the default ten minutes.
	export HELM_TIMEOUT="${HELM_TIMEOUT:-5m}"

	mark_audit_log
	echo "# Deploying kata-deploy in daemonset mode with SELinux confinement..." >&3
	deploy_kata "" --set deploymentMode=daemonset --set selinux.enabled=true
}

@test "The loader finds every domain the stages ask for in the node's policy" {
	assert_domains_resolve
}

@test "The policy module is loaded into the node's policy store" {
	assert_module_still_loaded
}

@test "The install runs in the union domain" {
	# kube-kata stays alive behind its health probes, so unlike the job-mode
	# stages its domain can be read straight off the node.
	run kata_deploy_process_contexts
	echo "# kata-deploy process contexts: ${output}" >&3
	[ "${status}" -eq 0 ]
	[[ "${output}" == *"kata_deploy_t"* ]]
	[[ "${output}" != *":container_t:"* ]]
	[[ "${output}" != *":spc_t:"* ]]
}

@test "The confined install does its host work" {
	assert_artifacts_installed
}

@test "The confined install logged no AVC denials" {
	assert_no_kata_deploy_denials "the daemonset-mode install"
}

@test "The confined uninstall reverts the node, and leaves the module loaded" {
	mark_audit_log
	uninstall_kata
	kubectl wait nodes --timeout=300s --all --for condition=Ready=True

	assert_artifacts_removed
	assert_no_kata_deploy_denials "the daemonset-mode uninstall"
	assert_module_still_loaded
}

# Last, and in the suite the union runs last, so the node is left as it was found
# and neither the tests above nor the other suite see a store without the module.
@test "The removal semodule -r documents takes the module off the node" {
	run remove_policy_module
	echo "# semodule -r kata-deploy: ${output}" >&3
	[ "${status}" -eq 0 ]

	assert_module_removed
}

teardown_file() {
	uninstall_kata 2>/dev/null || true
}
