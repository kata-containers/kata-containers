#!/bin/bash
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Shared helpers for the kata-deploy SELinux suites.

# status, output and BATS_FILE_TMPDIR are bats'; HELM_NAMESPACE is helm-deploy.bash'.
# shellcheck disable=SC2154

AUDIT_LOG="/host/var/log/audit/audit.log"
KATA_INSTALL_DIR="/host/opt/kata"

selinux_enforce() {
	run_on_host 'cat /host/sys/fs/selinux/enforce 2>/dev/null || echo none'
}

skip_unless_enforcing_node() {
	run selinux_enforce
	echo "# SELinux enforce: ${output}" >&3
	if [[ "${output}" != *"1"* ]]; then
		skip "the installer's SELinux confinement needs a node with SELinux enforcing"
	fi

	# Without the log the denial checks would pass without having looked.
	if ! run_on_host 'test -r /host/var/log/audit/audit.log'; then
		skip "no readable ${AUDIT_LOG}: auditd has to be running for denials to land there"
	fi
}

# A chart with no confinement to render runs the stages as container_t, which
# fails the same way a missing rule does but only after the install has retried
# and helm has timed out. Say so up front instead.
assert_chart_supports_selinux() {
	run helm template kata-deploy "$(get_chart_path)" --set selinux.enabled=true
	[[ "${status}" -eq 0 ]]
	if [[ "${output}" != *"install-stage-selinux-policy"* ]]; then
		echo "# the chart renders no selinux-policy stage: selinux.enabled is not supported here" >&3
		return 1
	fi
}

policy_module_path() {
	run_on_host 'ls -d /host/var/lib/selinux/*/active/modules/*/kata-deploy 2>/dev/null | head -1'
}

# The loader's own selector, so this works for the DaemonSet and Job pods alike.
policy_loader_log() {
	kubectl -n "${HELM_NAMESPACE}" logs -l "$(kata_deploy_pod_selector)" \
		-c selinux-policy --tail=-1 2>/dev/null || true
}

audit_mark_file() {
	echo "${BATS_FILE_TMPDIR}/audit-offset"
}

mark_audit_log() {
	run run_on_host 'wc -c < /host/var/log/audit/audit.log'
	[[ "${status}" -eq 0 ]]
	echo "${output//[^0-9]/}" > "$(audit_mark_file)"
}

# Scoped to kata_deploy and to the window since mark_audit_log, so neither
# another workload nor an earlier run can fail a suite.
denials_since_mark() {
	local offset
	offset=$(cat "$(audit_mark_file)")
	run_on_host "tail -c +$((offset + 1)) /host/var/log/audit/audit.log | grep 'avc: *denied' | grep kata_deploy || true"
}

# Diagnostics: a stage whose label did not apply is denied as container_t, which
# the scoped grep cannot tell apart from another container's noise.
all_denials_since_mark() {
	local offset
	offset=$(cat "$(audit_mark_file)")
	run_on_host "tail -c +$((offset + 1)) /host/var/log/audit/audit.log | grep 'avc: *denied' || true"
}

assert_no_kata_deploy_denials() {
	local what="${1}"

	run all_denials_since_mark
	echo "# All denials during ${what}: ${output:-none}" >&3

	run denials_since_mark
	echo "# kata_deploy denials during ${what}: ${output:-none}" >&3
	[[ "${status}" -eq 0 ]]
	[[ -z "${output}" ]]
}

# The loader logs this only after checking. When it cannot check it warns and
# says nothing, so requiring the line makes an unverified load a failure.
assert_domains_resolve() {
	run policy_loader_log
	echo "# selinux-policy stage log: ${output}" >&3
	[[ "${output}" == *"domains resolve"* ]]
}

assert_artifacts_installed() {
	run run_on_host "ls ${KATA_INSTALL_DIR}"
	echo "# ${KATA_INSTALL_DIR}: ${output}" >&3
	[[ "${status}" -eq 0 ]]
	[[ "${output}" == *"bin"* ]]
}

assert_artifacts_removed() {
	run run_on_host "test -e ${KATA_INSTALL_DIR} && echo PRESENT || echo GONE"
	echo "# ${KATA_INSTALL_DIR} after uninstall: ${output}" >&3
	[[ "${output}" == *"GONE"* ]]
}

# Removing the module is left to the admin: another release may still need it.
assert_module_still_loaded() {
	run policy_module_path
	echo "# Policy store entry: ${output}" >&3
	[[ -n "${output}" ]]
}

# What values.yaml tells the admin to run. Under a chroot with the host writable,
# as the installer does: the store belongs to the node, and so does its semodule.
remove_policy_module() {
	run_on_host 'chroot /host /usr/sbin/semodule -r kata-deploy || chroot /host /sbin/semodule -r kata-deploy' false
}

assert_module_removed() {
	run policy_module_path
	echo "# Policy store entry after removal: ${output:-none}" >&3
	[[ -z "${output}" ]]
}
