#!/usr/bin/env bats
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# BATS and the shared Kubernetes helpers provide these test globals.
# shellcheck disable=SC2154

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

host_allocated_bytes() {
	local path="$1"
	exec_host "${node}" "du -s -B1 '${path}' | awk 'NR == 1 {print \$1}'"
}

setup() {
	is_runtime_rs || skip "Direct volumes are tested only with runtime-rs"
	is_confidential_runtime_class && \
		skip "Direct volumes are not supported for confidential runtime classes"

	case "$(uname -m)" in
		x86_64 | aarch64) ;;
		*) skip "kata-ctl not shipped for $(uname -m); direct-volume test not exercised" ;;
	esac

	setup_common || die "setup_common failed"

	exec_host "${node}" \
		"for tool in mkfs.ext4 truncate; do command -v \${tool} >/dev/null || exit 1; done" || \
		die "Direct-volume filesystem tools are not installed on ${node}"

	pod_name="direct-volume-sparse"
	mountpoint="/data"
	volume_id="$(printf '%s' "${BATS_TEST_TMPDIR}" | sha256sum | cut -c1-12)"
	volume_path="/tmp/kata-direct-volume-${volume_id}"
	disk_path="${volume_path}.img"
	mount_info="{\"device\":\"${disk_path}\",\"volume-type\":\"directvol\",\"fstype\":\"ext4\",\"metadata\":{},\"options\":[]}"

	yaml_file="$(mktemp --tmpdir pod-direct-volume.XXXXXX.yaml)"
	cp "${pod_config_dir}/pod-direct-volume.yaml" "${yaml_file}"
	yq -i ".spec.volumes[0].hostPath.path = \"${volume_path}\"" "${yaml_file}"
	set_node "${yaml_file}" "${node}"

	mount_probe="awk -v mp='${mountpoint}' '\$5 == mp {for (i = 1; i <= NF; i++) if (\$i == \"-\") {print \$(i + 1), \$(i + 2); exit}}' /proc/self/mountinfo"
}

@test "Direct volume backing file remains sparse" {
	local allocated_after
	local allocated_before
	local apparent_size
	local fs_type
	local mount_source

	exec_host "${node}" "set -e; \
		rm -rf '${volume_path}' '${disk_path}'; \
		mkdir -p '${volume_path}'; \
		truncate -s 256M '${disk_path}'; \
		mkfs.ext4 -F -q '${disk_path}'; \
		/opt/kata/bin/kata-ctl direct-volume add '${volume_path}' '${mount_info}'; \
		sync; \
		echo direct-volume-ready" | grep -qx direct-volume-ready

	apparent_size="$(exec_host "${node}" "stat -c %s '${disk_path}'")"
	allocated_before="$(host_allocated_bytes "${disk_path}")"
	info "direct-volume apparent bytes before attach: ${apparent_size}"
	info "direct-volume allocated bytes before attach: ${allocated_before}"
	(( allocated_before < apparent_size / 2 )) || die "Test backing file is not sparse before attach"

	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	read -r fs_type mount_source < <(pod_exec "${pod_name}" sh -c "${mount_probe}")
	info "direct-volume guest mount: fs=${fs_type}, source=${mount_source}"
	[[ "${fs_type}" == "ext4" ]]
	[[ "${mount_source}" == /dev/* ]]

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
			rm -rf '${volume_path}' '${disk_path}'"
	fi

	rm -f "${yaml_file:-}"
}
