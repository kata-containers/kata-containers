#!/usr/bin/env bash
#
# Copyright (c) 2026 NVIDIA Corporation
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# These helpers run inside BATS tests that provide the per-test globals
# (pod_name, volume_name, mountpoint, yaml_file, policy_settings_dir) and source
# the shared test helpers that export the rest (node, wait_time, sleep_time,
# timeout), so they are intentionally referenced but not assigned here.
# shellcheck disable=SC2154

pod_evicted() {
	kubectl get pod "${pod_name}" -o jsonpath='{.status.reason}' 2>/dev/null | grep -q '^Evicted$'
}

pod_eviction_message() {
	kubectl get pod "${pod_name}" -o jsonpath='{.status.message}' 2>/dev/null || true
}

pod_events() {
	kubectl get events \
		--field-selector "involvedObject.kind=Pod,involvedObject.name=${pod_name}" \
		-o jsonpath='{range .items[*]}{.reason}{" "}{.message}{"\n"}{end}' 2>/dev/null || true
}

host_emptydir_volume_path() {
	local pod_uid
	pod_uid="$(kubectl get pod "${pod_name}" -o jsonpath='{.metadata.uid}')"
	echo "$(get_kubelet_data_dir)/pods/${pod_uid}/volumes/kubernetes.io~empty-dir/${volume_name}"
}

host_emptydir_usage_path() {
	local disk_path
	local volume_path

	volume_path="$(host_emptydir_volume_path)"
	disk_path="${volume_path}/disk.img"
	if exec_host "${node}" "test -f '${disk_path}'" >/dev/null 2>&1; then
		echo "${disk_path}"
	else
		echo "${volume_path}"
	fi
}

host_emptydir_allocated_bytes() {
	local host_path="$1"
	exec_host "${node}" "du -s -B1 '${host_path}' | awk 'NR == 1 {print \$1}'"
}

size_limit_eviction_observed() {
	local events
	local message
	local size_limit_pattern="(exceed|exceeded|exceeds).*(emptyDir|ephemeral|limit|sizeLimit|${volume_name})"

	if pod_evicted; then
		return 0
	fi

	message="$(pod_eviction_message)"
	if echo "${message}" | grep -Eqi "${size_limit_pattern}"; then
		return 0
	fi

	events="$(pod_events)"
	echo "${events}" | grep -Eqi "evict" || return 1
	echo "${events}" | grep -Eqi "${size_limit_pattern}"
}

host_emptydir_exceeds_allocated_bytes_or_pod_evicted() {
	local allocated_bytes
	local host_path
	local min_allocated_bytes="$1"

	if pod_evicted; then
		info "pod ${pod_name} is already evicted while waiting for sizeLimit accounting: $(pod_eviction_message)"
		return 0
	fi

	host_path="$(host_emptydir_usage_path)"
	if ! exec_host "${node}" "test -e '${host_path}'" >/dev/null 2>&1; then
		info "host emptyDir path is not present while waiting for sizeLimit accounting: ${host_path}"
		if pod_evicted; then
			info "pod ${pod_name} was evicted after host emptyDir path disappeared: $(pod_eviction_message)"
			return 0
		fi

		pod_evicted
		return $?
	fi

	allocated_bytes="$(host_emptydir_allocated_bytes "${host_path}" 2>/dev/null || echo 0)"
	info "host emptyDir allocated bytes while waiting for sizeLimit accounting: ${allocated_bytes}"
	[[ "${allocated_bytes}" =~ ^[0-9]+$ ]] || return 1
	(( allocated_bytes > min_allocated_bytes )) || pod_evicted
}

set_emptydir_size_limit() {
	local size_limit="$1"

	yq -i "(.spec.volumes[] | select(.name == \"${volume_name}\").emptyDir.sizeLimit) = \"${size_limit}\"" "${yaml_file}"
}

apply_emptydir_pod() {
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
	kubectl apply -f "${yaml_file}"
}

# Run the shared emptyDir sizeLimit eviction scenario.
#
# Callers set these globals first; they are the per-test inputs:
#   pod_name            - pod under test
#   volume_name         - emptyDir volume whose sizeLimit is exercised
#   mountpoint          - in-guest mount path of that volume
#   yaml_file           - pod manifest to mutate and apply
#   policy_settings_dir - genpolicy settings dir for this test
#
# The size limit, write size and eviction wait are identical for every backend,
# so they stay internal here instead of being passed in by each test.
run_emptydir_size_limit_eviction_test() {
	local accounting_wait_time="${wait_time}"
	local eviction_wait_time=120
	local events
	local size_limit="512Mi"
	local size_limit_bytes=$((512 * 1024 * 1024))
	local write_command
	local write_mib=2048

	write_command="dd if=/dev/zero of='${mountpoint}/size-limit-test.bin' bs=1M count=${write_mib} conv=fsync"
	add_exec_to_policy_settings "${policy_settings_dir}" sh -c "${write_command}"

	set_emptydir_size_limit "${size_limit}"
	apply_emptydir_pod
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	pod_exec "${pod_name}" sh -c "${write_command}" || true

	# First wait for proof that the oversized write crossed the emptyDir limit:
	# either the host disk image exceeds the limit, or kubelet already evicted the
	# pod and may have removed disk.img. This distinguishes "we did not write too
	# much" from "we wrote too much, but eviction evidence did not appear".
	waitForProcess "${accounting_wait_time}" "${sleep_time}" \
		"host_emptydir_exceeds_allocated_bytes_or_pod_evicted '${size_limit_bytes}'"

	# Eviction has been observed to take slightly more than the generic 90s wait.
	waitForProcess "${eviction_wait_time}" "${sleep_time}" size_limit_eviction_observed
	info "pod ${pod_name} reason: $(kubectl get pod "${pod_name}" -o jsonpath='{.status.reason}' 2>/dev/null || true)"
	info "pod ${pod_name} eviction message: $(pod_eviction_message)"

	events="$(pod_events)"
	info "events for ${pod_name}: ${events}"
}
