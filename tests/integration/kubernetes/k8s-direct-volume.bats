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

# exec_host succeeds even when du fails, and bash would read the empty output
# as 0 in the size comparisons, so reject anything that is not a number.
host_allocated_bytes() {
	local path="$1"
	local bytes

	bytes="$(exec_host "${node}" "du -s -B1 '${path}' | awk 'NR == 1 {print \$1}'")"
	[[ "${bytes}" =~ ^[0-9]+$ ]] || \
		die "Cannot measure the allocated bytes of ${path} on ${node}: '${bytes}'"
	echo "${bytes}"
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

require_qcow2_support() {
	# The other hypervisors reject qcow2 direct volumes by design.
	[[ "${KATA_HYPERVISOR}" == qemu*runtime-rs ]] || \
		skip "qcow2 direct volumes are supported only by runtime-rs with QEMU"
	exec_host "${node}" "command -v qemu-img >/dev/null" || \
		die "Direct-volume qcow2 tool qemu-img is not installed on ${node}"
}

# Register a thin qcow2 direct volume whose filesystem holds the given marker.
# The filesystem is built in a raw image, which is then converted to qcow2.
# With "unmap" as the discard mode, QEMU frees the clusters the guest discards,
# and the guest mounts the filesystem with "discard" so that it sends them.
prepare_qcow2_volume() {
	local marker="$1"
	local discard="${2:-}"
	local info
	local metadata='{"io.katacontainers.volume.format": "qcow2"}'
	local options='[]'

	if [[ -n "${discard}" ]]; then
		metadata="$(jq -cn --arg discard "${discard}" \
			'{"io.katacontainers.volume.format": "qcow2", "io.katacontainers.volume.discard": $discard}')"
		options='["discard"]'
	fi

	exec_host "${node}" "set -e; \
		rm -rf '${volume_path}' '${disk_path}' '${base_path}' '${seed_path}'; \
		mkdir -p '${volume_path}' '${seed_path}'; \
		printf '%s\n' '${marker}' > '${seed_path}/dav-backing-marker'; \
		truncate -s 64M '${base_path}'; \
		mkfs.ext4 -F -q -d '${seed_path}' '${base_path}'; \
		rm -rf '${seed_path}'; \
		qemu-img convert -q -f raw -O qcow2 '${base_path}' '${disk_path}'; \
		rm -f '${base_path}'; \
		echo qcow2-ready" | grep -qx qcow2-ready

	info="$(jq -cn --arg device "${disk_path}" --argjson metadata "${metadata}" \
		--argjson options "${options}" \
		'{device: $device, "volume-type": "directvol", fstype: "ext4", metadata: $metadata, options: $options}')"

	exec_host "${node}" \
		"/opt/kata/bin/kata-ctl direct-volume add '${volume_path}' '${info}' && echo direct-volume-ready" | \
		grep -qx direct-volume-ready
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
	base_path="${volume_path}.base.img"
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

@test "DAV mounts a thin qcow2 volume" {
	local backing_marker="dav-qcow2-thin-${volume_id}"
	local allocated_after
	local allocated_before
	local virtual_size=$((64 * 1024 * 1024))

	require_qcow2_support
	prepare_qcow2_volume "${backing_marker}"
	allocated_before="$(host_allocated_bytes "${disk_path}")"
	info "thin qcow2 allocated bytes before attach: ${allocated_before} of ${virtual_size}"
	(( allocated_before < virtual_size / 2 )) || die "Test qcow2 image is not thin before attach"

	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"
	assert_guest_direct_volume "${backing_marker}"
	pod_exec "${pod_name}" sh -c \
		"printf '%s\n' direct-volume-ready > '${mountpoint}/probe' && sync && \
		grep -qx direct-volume-ready '${mountpoint}/probe'"

	exec_host "${node}" sync
	allocated_after="$(host_allocated_bytes "${disk_path}")"
	info "thin qcow2 allocated bytes after attach: ${allocated_after}"
	info "thin qcow2 allocation growth after attach: $((allocated_after - allocated_before))"
	# Allow filesystem metadata growth while rejecting full-image preallocation.
	(( allocated_after < virtual_size / 2 )) || \
		die "qcow2 direct volume was preallocated during attach"
}

@test "DAV qcow2 volume with discard unmap returns deleted space to the host" {
	local backing_marker="dav-qcow2-unmap-${volume_id}"
	local allocated_empty
	local allocated_freed
	local allocated_full
	local data_bytes=$((32 * 1024 * 1024))
	local deadline

	require_qcow2_support
	prepare_qcow2_volume "${backing_marker}" unmap

	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"
	assert_guest_direct_volume "${backing_marker}"

	exec_host "${node}" sync
	allocated_empty="$(host_allocated_bytes "${disk_path}")"
	info "qcow2 allocated bytes before the guest writes: ${allocated_empty}"

	pod_exec "${pod_name}" sh -c \
		"dd if=/dev/urandom of='${mountpoint}/fill' bs=1M count=$((data_bytes / 1024 / 1024)) 2>/dev/null && sync"
	exec_host "${node}" sync
	allocated_full="$(host_allocated_bytes "${disk_path}")"
	info "qcow2 allocated bytes after the guest wrote ${data_bytes}: ${allocated_full}"
	(( allocated_full - allocated_empty >= data_bytes / 2 )) || \
		die "Guest writes did not grow the qcow2 image"

	# The filesystem is mounted with "discard", so the guest discards the
	# file's blocks once the deletion is committed, which sync forces.
	pod_exec "${pod_name}" sh -c "rm -f '${mountpoint}/fill' && sync"

	deadline=$((SECONDS + 60))
	while :; do
		exec_host "${node}" sync
		allocated_freed="$(host_allocated_bytes "${disk_path}")"
		(( allocated_full - allocated_freed >= data_bytes / 2 )) && break
		(( SECONDS < deadline )) || \
			die "Deleted guest data still allocated in the qcow2 image: ${allocated_freed} bytes"
		sleep 5
	done
	info "qcow2 allocated bytes after the guest deleted the data: ${allocated_freed}"
}

teardown() {
	if [[ -n "${node:-}" ]]; then
		teardown_common "${node}" "${node_start_time:-}" || true
	fi

	if [[ -n "${node:-}" && -n "${volume_path:-}" ]]; then
		exec_host "${node}" \
			"/opt/kata/bin/kata-ctl direct-volume remove '${volume_path}' >/dev/null 2>&1 || true; \
			rm -rf '${volume_path}' '${disk_path}' '${base_path}' '${seed_path}'"
	fi

	rm -f "${yaml_file:-}"
}
