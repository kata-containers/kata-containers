#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# The installer's SELinux confinement in job mode, on a real enforcing node.
#
# This mode runs each stage in a container of its own, so each gets a domain of
# its own. Unconfined, they run as container_t and the install dies in AVC
# denials (#13751).
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

NODE_BINARY="/host/usr/local/bin/mkfs.erofs"

# udev_rules_t, which only the confined remove-artifacts stage ever touches: the
# privileged stage that writes this rule raises no denial, so an uninstall is the
# only place a missing rule for it shows up.
ROOTLESS_UDEV_RULE="/host/etc/udev/rules.d/99-kata-containers-rootless-default.rules"

# Whichever shim is under test, qemu-runtime-rs is enabled on every architecture,
# so the rule gets written wherever this runs.
ROOTLESS_VALUES=(--set shims.qemu-runtime-rs.hypervisor.rootless=true)

# Configured directly rather than through the EROFS snapshotter, so the domain
# is covered wherever this runs.
NODE_BINARIES_VALUES=(
	--set 'nodeBinaries.erofs-utils.image=quay.io/kata-containers/erofs-utils:1.9.3'
	--set 'nodeBinaries.erofs-utils.binaries[0]=mkfs.erofs'
)

# The default reaps the per-node Job pods while the suite is still reading them.
JOB_TTL=3600

setup_file() {
	skip_unless_enforcing_node
	ensure_helm
	assert_chart_supports_selinux

	mark_audit_log
	echo "# Deploying kata-deploy in job mode with SELinux confinement..." >&3
	# A denial is deterministic, so per-node retries only delay the report.
	deploy_kata "" \
		--set deploymentMode=job \
		--set selinux.enabled=true \
		--set job.backoffLimit=0 \
		--set "job.ttlSecondsAfterFinished=${JOB_TTL}" \
		"${NODE_BINARIES_VALUES[@]}" \
		"${ROOTLESS_VALUES[@]}"
	show_policy_loader_log
}

@test "The policy module is loaded into the node's policy store" {
	assert_module_still_loaded
}

@test "The confined stages do their host work" {
	assert_artifacts_installed
}

@test "The nodeBinaries stage writes the node's /usr/local/bin" {
	# bin_t, which kata_deploy_node_binaries_t alone may write, and reaching it
	# means the stage also took the node mutation lock under var_lock_t.
	run run_on_host "test -x ${NODE_BINARY} && echo INSTALLED || echo MISSING"
	echo "# ${NODE_BINARY}: ${output}" >&3
	[[ "${output}" == *"INSTALLED"* ]]
}

@test "The rootless stage leaves the udev rule the cleanup has to remove" {
	# Asserted before the uninstall so the removal below cannot pass by removing
	# nothing, which is what a missing udev_rules_t rule would look like.
	run run_on_host "test -e ${ROOTLESS_UDEV_RULE} && echo PRESENT || echo MISSING"
	echo "# ${ROOTLESS_UDEV_RULE}: ${output}" >&3
	[[ "${output}" == *"PRESENT"* ]]
}

@test "The confined install logged no AVC denials" {
	assert_no_kata_deploy_denials "the job-mode install"
}

@test "The confined cleanup stages uninstall, and leave the module loaded" {
	mark_audit_log
	uninstall_kata
	kubectl wait nodes --timeout=300s --all --for condition=Ready=True

	assert_artifacts_removed

	# udev_rules_t: confined removal, and the reason kata_deploy_artifacts_t is
	# granted it at all.
	run run_on_host "test -e ${ROOTLESS_UDEV_RULE} && echo PRESENT || echo GONE"
	echo "# ${ROOTLESS_UDEV_RULE} after uninstall: ${output}" >&3
	[[ "${output}" == *"GONE"* ]]

	assert_no_kata_deploy_denials "the job-mode uninstall"
	assert_module_still_loaded
}

teardown_file() {
	uninstall_kata 2>/dev/null || true
}
