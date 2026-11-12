#!/usr/bin/env bats
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# This suite exercises the DAV interface independently of a CSI driver.
# It uses the kata-ctl tool's 'direct-volume add' capability to register
# the DAV mountInfo.json under /run/kata-containers/shared/direct-volumes/.
# The suite's pod manifest uses hostPath to create a bind mount for which
# the shim will find the corresponding mountInfo.json file.

# BATS and the shared Kubernetes helpers provide these test globals.
# shellcheck disable=SC2154

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

host_allocated_bytes() {
	local path="$1"
	exec_host "${node}" "du -s -B1 '${path}' | awk 'NR == 1 {print \$1}'"
}

prepare_direct_volume() {
	local size="$1"
	local marker="${2:-}"

	exec_host "${node}" "set -e; \
		rm -rf '${volume_path}' '${disk_path}' '${seed_path}'; \
		mkdir -p '${volume_path}'; \
		truncate -s '${size}' '${disk_path}'; \
		if [ -n '${marker}' ]; then \
			mkdir -p '${seed_path}'; \
			printf '%s\n' '${marker}' > '${seed_path}/dav-backing-marker'; \
			mkfs.ext4 -F -q -d '${seed_path}' '${disk_path}'; \
			rm -rf '${seed_path}'; \
		else \
			mkfs.ext4 -F -q '${disk_path}'; \
		fi; \
		/opt/kata/bin/kata-ctl direct-volume add '${volume_path}' '${mount_info}'; \
		sync; \
		echo direct-volume-ready" | grep -qx direct-volume-ready
}

assert_guest_direct_volume() {
	local fs_type
	local marker="${1:-}"
	local mount_source

	read -r fs_type mount_source < <(pod_exec "${pod_name}" sh -c "${mount_probe}")
	info "direct-volume guest mount: fs=${fs_type}, source=${mount_source}"
	[[ "${fs_type}" == "ext4" ]]
	[[ "${mount_source}" == /dev/* ]]
	if [[ -n "${marker}" ]]; then
		pod_exec "${pod_name}" sh -c \
			"grep -qx '${marker}' '${mountpoint}/dav-backing-marker'"
	fi
}

setup() {
	is_runtime_rs || skip "DAV interface is tested only with runtime-rs"
	auto_generate_policy_enabled && \
		skip "genpolicy does not know about how requests change when DAV is used"

	case "$(uname -m)" in
		x86_64 | aarch64) ;;
		*) skip "kata-ctl not shipped for $(uname -m); direct-volume test not exercised" ;;
	esac

	setup_common || die "setup_common failed"

	exec_host "${node}" \
		"for tool in mkfs.ext4 truncate; do command -v \${tool} >/dev/null || exit 1; done" || \
		die "Direct-volume filesystem tools are not installed on ${node}"
	exec_host "${node}" "test -x /opt/kata/bin/kata-ctl" || \
		die "kata-ctl is not installed on ${node}; install the kata-tools bundle"

	pod_name="direct-volume"
	mountpoint="/data"
	volume_id="$(printf '%s' "${BATS_TEST_TMPDIR}" | sha256sum | cut -c1-12)"
	volume_path="/tmp/kata-direct-volume-${volume_id}"
	disk_path="${volume_path}.img"
	seed_path="${volume_path}.seed"
	mount_info="{\"device\":\"${disk_path}\",\"volume-type\":\"directvol\",\"fstype\":\"ext4\",\"metadata\":{},\"options\":[]}"

	yaml_file="$(mktemp --tmpdir pod-direct-volume.XXXXXX.yaml)"
	cp "${pod_config_dir}/pod-direct-volume.yaml" "${yaml_file}"
	yq -i ".spec.volumes[0].hostPath.path = \"${volume_path}\"" "${yaml_file}"
	set_node "${yaml_file}" "${node}"

	mount_probe="awk -v mp='${mountpoint}' '\$5 == mp {for (i = 1; i <= NF; i++) if (\$i == \"-\") {print \$(i + 1), \$(i + 2); exit}}' /proc/self/mountinfo"
}

@test "DAV mounts a registered filesystem read/write in the guest" {
	local backing_marker="dav-backing-${volume_id}"

	prepare_direct_volume 64M "${backing_marker}"
	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"
	assert_guest_direct_volume "${backing_marker}"
	pod_exec "${pod_name}" sh -c \
		"printf '%s\n' direct-volume-ready > '${mountpoint}/probe' && \
		grep -qx direct-volume-ready '${mountpoint}/probe'"
}

@test "Direct volume backing file remains sparse" {
	local allocated_after
	local allocated_before
	local apparent_size

	prepare_direct_volume 256M

	apparent_size="$(exec_host "${node}" "stat -c %s '${disk_path}'")"
	allocated_before="$(host_allocated_bytes "${disk_path}")"
	info "direct-volume apparent bytes before attach: ${apparent_size}"
	info "direct-volume allocated bytes before attach: ${allocated_before}"
	(( allocated_before < apparent_size / 2 )) || die "Test backing file is not sparse before attach"

	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"
	assert_guest_direct_volume

	exec_host "${node}" sync
	allocated_after="$(host_allocated_bytes "${disk_path}")"
	info "direct-volume allocated bytes after attach: ${allocated_after}"
	info "direct-volume allocation growth after attach: $((allocated_after - allocated_before))"
	# Allow filesystem metadata growth while rejecting full-image preallocation.
	(( allocated_after < apparent_size / 2 )) || \
		die "Direct-volume backing file was preallocated during attach"
}

teardown() {
	if [[ -n "${node:-}" ]]; then
		teardown_common "${node}" "${node_start_time:-}" || true
	fi

	if [[ -n "${node:-}" && -n "${volume_path:-}" ]]; then
		exec_host "${node}" \
			"/opt/kata/bin/kata-ctl direct-volume remove '${volume_path}' >/dev/null 2>&1 || true; \
			rm -rf '${volume_path}' '${disk_path}' '${seed_path}'"
	fi

	rm -f "${yaml_file:-}"
}
