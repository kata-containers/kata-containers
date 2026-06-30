#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/emptydir_common.sh"

export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

skip_unsupported_runtime() {
	# The runtime-rs block emptyDir path uses BlockModern, inherited from the
	# older block-encrypted implementation. The encrypted test is CoCo-only, so
	# it never reached clh-runtime-rs or dragonball in CI; this generic
	# block-plain test would, so skip those unsupported VMM paths explicitly.
	case "${KATA_HYPERVISOR}" in
		clh-runtime-rs|clh-azure-runtime-rs|dragonball)
			skip "block-plain emptyDir uses runtime-rs BlockModern, whose VMM glue is missing for ${KATA_HYPERVISOR}"
			;;
	esac
}

# Return the full mountinfo row whose field 5 is the tested mount point.
mountinfo_for_mountpoint() {
	pod_exec "${pod_name}" sh -c \
		"awk -v mp='${mountpoint}' '\$5 == mp {print; exit}' /proc/self/mountinfo"
}

# Return a field after the "-" separator: 1 = fs type, 2 = source, 3 = super options.
mountinfo_after_separator_field() {
	local field_offset="$1"

	pod_exec "${pod_name}" sh -c \
		"awk -v mp='${mountpoint}' '\$5 == mp {for (i = 1; i <= NF; i++) if (\$i == \"-\") {print \$(i + ${field_offset}); exit}}' /proc/self/mountinfo"
}

# Return field 6 mount options plus super options, which are three fields after "-".
mount_options() {
	pod_exec "${pod_name}" sh -c \
		"awk -v mp='${mountpoint}' '\$5 == mp {for (i = 1; i <= NF; i++) if (\$i == \"-\") {print \$6 \",\" \$(i + 3); exit}}' /proc/self/mountinfo"
}

host_disk_path() {
	echo "$(host_emptydir_volume_path)/disk.img"
}

guest_discard_max_bytes() {
	pod_exec "${pod_name}" sh -c \
		"src=\$(awk -v mp='${mountpoint}' '\$5 == mp {for (i = 1; i <= NF; i++) if (\$i == \"-\") {print \$(i + 2); exit}}' /proc/self/mountinfo); \
		blk=\${src#/dev/}; blk=\${blk##*/}; \
		if [ -e \"/sys/class/block/\${blk}/partition\" ]; then blk=\$(basename \"\$(readlink -f \"/sys/class/block/\${blk}/..\")\"); fi; \
		cat \"/sys/class/block/\${blk}/queue/discard_max_bytes\" 2>/dev/null || echo 0"
}

setup() {
	local runtime_config_dropin_file

	skip_unsupported_runtime
	setup_common || die "setup_common failed"

	pod_name="plain-ephemeral-data-storage"
	volume_name="temp-plain"
	mountpoint="/mnt/temp-plain"
	yaml_template="${pod_config_dir}/pod-plain-ephemeral-data-storage.yaml.in"
	yaml_file="${pod_config_dir}/pod-plain-ephemeral-data-storage.yaml"

	RUNTIMECLASS="kata-${KATA_HYPERVISOR}" envsubst "\${RUNTIMECLASS}" \
		< "${yaml_template}" > "${yaml_file}"

	runtime_config_dropin_file="${BATS_FILE_TMPDIR}/99-k8s-plain-ephemeral-data-storage.toml"
	cat > "${runtime_config_dropin_file}" <<EOF
[runtime]
emptydir_mode = "block-plain"
EOF
	runtime_config_dropin="$(set_kata_runtime_config_dropin_file \
		"${node}" \
		"${runtime_config_dropin_file}")" || \
		skip "No Kata runtime config found for ${KATA_HYPERVISOR}"

	if [[ "${PULL_TYPE:-default}" == "guest-pull" ]] && is_confidential_runtime_class "${KATA_HYPERVISOR}"; then
		set_metadata_annotation "${yaml_file}" \
			"io.containerd.cri.runtime-handler" \
			"kata-${KATA_HYPERVISOR}"
	fi

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	set_genpolicy_emptydir_type "${policy_settings_dir}" "block-plain"
	allow_requests "${policy_settings_dir}" "ExecProcessRequest" "ReadStreamRequest"
}

@test "Plain ephemeral data storage" {
	apply_emptydir_pod
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	# Verify that emptyDir is mounted as a plain ext4 block device.
	mountinfo="$(mountinfo_for_mountpoint)"
	info "mountinfo for ${mountpoint}: ${mountinfo}"
	[[ -n "${mountinfo}" ]]

	fs_type="$(mountinfo_after_separator_field 1)"
	source="$(mountinfo_after_separator_field 2)"
	info "mount source: ${source}"
	info "mount filesystem type: ${fs_type}"

	[[ "${fs_type}" == "ext4" ]]
	[[ "${source}" == /dev/* ]]
	[[ "${source}" != /dev/mapper/* ]]
	[[ "${source}" != "tmpfs" ]]

	[[ "$(pod_exec "${pod_name}" sh -c "echo foo > '${mountpoint}/foo.txt' && cat '${mountpoint}/foo.txt'")" == "foo" ]]
	pod_exec "${pod_name}" sh -c "dd if=/dev/zero of='${mountpoint}/blob' bs=1M count=32 && sync"
}

@test "Plain ephemeral data storage sizeLimit evicts pod" {
	run_emptydir_size_limit_eviction_test
}

@test "Plain ephemeral data storage discard reclaims host blocks" {
	local allocated_after_delete
	local allocated_after_write
	local allocated_before
	local apparent_size
	local discard_max
	local disk_path
	local min_delta
	local options

	apply_emptydir_pod
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	# Reclaim requires discard support in the guest and discard on the mount.
	discard_max="$(guest_discard_max_bytes | tr -d '\r\n')"
	[[ "${discard_max}" =~ ^[0-9]+$ ]] || discard_max=0
	(( discard_max > 0 )) || skip "Block device for ${mountpoint} does not advertise discard support"

	options="$(mount_options)"
	info "mount options for ${mountpoint}: ${options}"
	[[ ",${options}," == *",discard,"* ]]

	# Track the sparse host disk image backing this emptyDir volume.
	disk_path="$(host_disk_path)"
	exec_host "${node}" "test -f '${disk_path}'" || die "Missing host disk image ${disk_path}"

	apparent_size="$(exec_host "${node}" "stat -c %s '${disk_path}'")"
	allocated_before="$(host_emptydir_allocated_bytes "${disk_path}")"
	info "host disk apparent size before write: ${apparent_size}"
	info "host disk allocated bytes before write: ${allocated_before}"
	(( allocated_before < apparent_size ))

	# Write enough data to force host allocation growth.
	pod_exec "${pod_name}" sh -c "dd if=/dev/zero of='${mountpoint}/discard-test.bin' bs=1M count=96 conv=fsync"
	exec_host "${node}" sync

	allocated_after_write="$(host_emptydir_allocated_bytes "${disk_path}")"
	info "host disk allocated bytes after write: ${allocated_after_write}"

	min_delta=$((32 * 1024 * 1024))
	(( allocated_after_write >= allocated_before + min_delta ))

	# Removing the guest file should discard blocks from the host image.
	wait_for_host_disk_reclaim() {
		allocated_after_delete="$(host_emptydir_allocated_bytes "${disk_path}")"
		info "host disk allocated bytes after delete: ${allocated_after_delete}"
		(( allocated_after_delete <= allocated_after_write - min_delta ))
	}

	pod_exec "${pod_name}" sh -c "rm -f '${mountpoint}/discard-test.bin' && sync"
	exec_host "${node}" sync

	waitForProcess "${wait_time}" "${sleep_time}" wait_for_host_disk_reclaim
}

teardown() {
	skip_unsupported_runtime

	echo "=== Plain ephemeral data storage pod describe ==="
	kubectl describe pod "${pod_name:-plain-ephemeral-data-storage}" || true

	echo "=== Plain ephemeral data storage pod logs ==="
	kubectl logs "${pod_name:-plain-ephemeral-data-storage}" || true

	# Always restore the Kata config (no-op if no drop-in was applied).
	remove_kata_runtime_config_dropin_file "${node}" "${runtime_config_dropin:-}" || true

	delete_tmp_policy_settings_dir "${policy_settings_dir:-}"

	[[ -f "${yaml_file:-}" ]] && kubectl delete -f "${yaml_file}" --ignore-not-found=true

	print_node_journal_since_test_start "${node}" "${node_start_time:-}" "${BATS_TEST_COMPLETED:-}"
}
