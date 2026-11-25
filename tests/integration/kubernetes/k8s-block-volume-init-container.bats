#!/usr/bin/env bats
#
# Copyright (c) NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# An init container and the main container of a pod use the same raw block
# volume. The shim unplugs the disk when the init container is gone and plugs
# it again for the main container, under the same index.
#
# The volume sits on device-mapper's delay target, so each test can make one
# kind of request slow and see what the shim does when the VMM takes longer
# to answer than the shim waits.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

dm_name="kata-test-slow-volume"

# Set how long reads, writes and flushes of the volume take, in milliseconds.
set_volume_delays() {
	local read_ms="$1"
	local write_ms="$2"
	local flush_ms="$3"
	local table="0 ${sectors} delay ${loop_dev} 0 ${read_ms} ${loop_dev} 0 ${write_ms} ${loop_dev} 0 ${flush_ms}"

	if exec_host "${node}" "dmsetup info ${dm_name} >/dev/null 2>&1"; then
		exec_host "${node}" "dmsetup suspend --noudevsync --nolockfs ${dm_name} && \
			dmsetup reload ${dm_name} --table '${table}' && \
			dmsetup resume --noudevsync ${dm_name}"
	else
		exec_host "${node}" "dmsetup create --noudevsync ${dm_name} --table '${table}' && \
			dmsetup mknodes ${dm_name}"
	fi
}

create_pod() {
	local init_command="$1"

	pod_config="$(mktemp --tmpdir pod-block-pv-init.XXXXX.yaml)"
	sed -e "s|INIT_COMMAND|${init_command}|" \
		"${pod_config_dir}/${pod_name}.yaml" > "${pod_config}"
	auto_generate_policy "${pod_config_dir}" "${pod_config}"
	kubectl create -f "${pod_config}"
}

main_container_restarts() {
	kubectl get pod "${pod_name}" -o jsonpath='{.status.containerStatuses[0].restartCount}'
}

# Warning events about the main container failing to be created or started.
# Events outlive the pod they are about, and every test uses the same pod name.
failed_events() {
	local uid

	uid="$(kubectl get pod "${pod_name}" -o jsonpath='{.metadata.uid}')"
	kubectl get events \
		--field-selector "involvedObject.uid=${uid},reason=Failed" \
		-o jsonpath='{range .items[*]}{.message}{"\n"}{end}'
}

# A disk that the VMM has let go of holds no descriptor on the volume.
sandbox_volume_descriptors() {
	local sandbox_id

	sandbox_id="$(get_pod_sandbox_id "${pod_name}")"
	[[ -n "${sandbox_id}" ]] || die "No sandbox ID found for pod ${pod_name}"
	# The descriptors name the device either way, depending on how it was
	# opened.
	exec_host "${node}" "for pid in \$(pgrep -f '${sandbox_id}'); do ls -l /proc/\${pid}/fd 2>/dev/null; done | \
		grep -c -E -- \"-> (/dev/mapper/${dm_name}|/dev/dm-\$(dmsetup info -c --noheadings -o minor ${dm_name}))\\\$\" || true"
}

wait_for_init_container() {
	local state="$1"
	local cmd="kubectl get pod ${pod_name} -o jsonpath='{.status.initContainerStatuses[0].state.${state}}' | grep -q ."

	waitForProcess 180 "${sleep_time}" "${cmd}"
}

setup() {
	[[ "${KATA_HYPERVISOR}" == qemu*-runtime-rs ]] || skip "block volume reuse is tested for QEMU on runtime-rs only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

	setup_common || die "setup_common failed"

	pod_name="pod-block-pv-init-container"
	volume_name="block-loop-pv"
	volume_claim="block-loop-pvc"
	vol_capacity="500M"

	# Only a node that answers and has no delay target is a reason to skip.
	dm_delay="$(exec_host "${node}" "modprobe dm-delay && command -v dmsetup >/dev/null; echo \$?")"
	[[ "${dm_delay}" =~ ^[0-9]+$ ]] || die "Could not run commands on ${node}: '${dm_delay}'"
	[[ "${dm_delay}" == "0" ]] || skip "device-mapper's delay target is not available on ${node}"

	disk_image="$(exec_host "${node}" mktemp --tmpdir disk.XXXXXX.img)"
	exec_host "${node}" truncate "${disk_image}" --size "${vol_capacity}"
	loop_dev="$(exec_host "${node}" losetup -f --show "${disk_image}")"
	sectors="$(exec_host "${node}" blockdev --getsz "${loop_dev}")"
	set_volume_delays 0 0 0

	kubectl create -f volume/local-storage.yaml
	pv_config="$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)"
	sed -e "s|LOOP_DEVICE|/dev/mapper/${dm_name}|" \
		-e "s|HOSTNAME|${node}|" \
		-e "s|CAPACITY|${vol_capacity}|" \
		volume/block-loop-pv.yaml > "${pv_config}"
	kubectl create -f "${pv_config}"
	pvc_config="$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)"
	sed -e "s|CAPACITY|${vol_capacity}|" volume/block-loop-pvc.yaml > "${pvc_config}"
	kubectl create -f "${pvc_config}"
}

# The VMM flushes a disk when it removes it. A flush that takes two seconds
# is not long, and the main container has to start on its first attempt.
@test "The main container starts after a slow flush of the init container's disk" {
	set_volume_delays 0 0 2000

	create_pod "dd if=/dev/zero of=/dev/xda bs=1M count=16"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	failed="$(failed_events)"
	[[ -z "${failed}" ]] || die "The main container failed before it started: ${failed}"
	restarts="$(main_container_restarts)"
	[[ "${restarts}" == "0" ]] || die "The main container was restarted ${restarts} time(s)"
}

# A flush that takes 45 seconds outlasts every attempt to hotunplug the init
# container's disk before that container is deleted. The disk still has to be
# released afterwards. Both containers use the same volume, so once the init
# container is gone the sandbox must hold only one disk reference to it, the
# main container's. If the init container's disk is never released, the
# sandbox keeps two.
@test "The init container's disk is released after a flush that outlasts its cleanup" {
	set_volume_delays 0 0 45000

	create_pod "dd if=/dev/zero of=/dev/xda bs=1M count=16; sleep 30"
	wait_for_init_container running
	one_disk="$(sandbox_volume_descriptors)"
	[[ "${one_disk}" -gt 0 ]] || die "The sandbox holds no descriptor on the init container's disk"

	kubectl wait --for=condition=Ready --timeout=300s "pod/${pod_name}"

	cmd="[[ \$(sandbox_volume_descriptors) -eq ${one_disk} ]]"
	waitForProcess 60 5 "${cmd}" || \
		die "The sandbox holds $(sandbox_volume_descriptors) descriptors on the volume, ${one_disk} are needed for one disk"
}

# The volume is slow to open while the main container is first created, so
# the VMM's answer comes after the shim stopped waiting for it.
# Once the storage is fast again the kubelet's next attempt has to succeed.
@test "The main container starts once a volume that was slow to open has recovered" {
	# With direct I/O the VMM reads from the volume when it opens it.
	local_dropin="$(mktemp --tmpdir 60-block-volume-direct.XXXXX.toml)"
	cat > "${local_dropin}" <<-EOF
		[hypervisor.qemu]
		block_device_cache_direct = true
	EOF
	runtime_config_dropin="$(set_kata_runtime_config_dropin_file "${node}" "${local_dropin}")" || \
		die "failed to write the direct I/O drop-in for ${KATA_HYPERVISOR}"

	create_pod "dd if=/dev/zero of=/dev/xda bs=1M count=4; sleep 20"
	wait_for_init_container running
	set_volume_delays 12000 0 0
	wait_for_init_container terminated

	# Making the volume fast before an attempt has failed on it would leave
	# nothing to recover from.
	cmd="[[ -n \"\$(failed_events)\" ]]"
	waitForProcess 180 5 "${cmd}" || \
		die "No attempt to create the main container failed while the volume was slow"
	set_volume_delays 0 0 0

	kubectl wait --for=condition=Ready --timeout=180s "pod/${pod_name}"
}

teardown() {
	[[ "${KATA_HYPERVISOR}" == qemu*-runtime-rs ]] || skip "block volume reuse is tested for QEMU on runtime-rs only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

	kubectl describe "pod/${pod_name}" || true
	kubectl get events --field-selector "involvedObject.name=${pod_name}" || true

	# Let pending requests through before the volume is taken apart.
	[[ -n "${loop_dev:-}" ]] && set_volume_delays 0 0 0 || true
	kubectl delete pod "${pod_name}" --ignore-not-found=true --wait=true --timeout="${timeout}"
	kubectl delete pvc "${volume_claim}" --ignore-not-found=true
	kubectl delete pv "${volume_name}" --ignore-not-found=true
	kubectl delete storageclass local-storage --ignore-not-found=true
	rm -f "${pv_config:-}" "${pvc_config:-}" "${pod_config:-}" "${local_dropin:-}"
	remove_kata_runtime_config_dropin_file "${node}" "${runtime_config_dropin:-}"

	if [[ -n "${loop_dev:-}" ]]; then
		# The kubelet lets go of the volume a little after the pod is gone.
		exec_host "${node}" "for i in \$(seq 1 30); do \
			dmsetup remove --noudevsync ${dm_name} 2>/dev/null && break; sleep 2; done; \
			losetup -d ${loop_dev}; rm -f ${disk_image}" || true
	fi

	teardown_common "${node}" "${node_start_time:-}"
}
