#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
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

setup() {
	is_runtime_rs || skip "block volume reuse is tested for runtime-rs only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

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

teardown() {
	is_runtime_rs || skip "block volume reuse is tested for runtime-rs only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

	kubectl describe "pod/${pod_name}" || true
	kubectl get events --field-selector "involvedObject.name=${pod_name}" || true

	# Let pending requests through before the volume is taken apart.
	[[ -n "${loop_dev:-}" ]] && set_volume_delays 0 0 0 || true
	kubectl delete pod "${pod_name}" --ignore-not-found=true --wait=true --timeout="${timeout}"
	kubectl delete pvc "${volume_claim}" --ignore-not-found=true
	kubectl delete pv "${volume_name}" --ignore-not-found=true
	kubectl delete storageclass local-storage --ignore-not-found=true
	rm -f "${pv_config:-}" "${pvc_config:-}" "${pod_config:-}"

	if [[ -n "${loop_dev:-}" ]]; then
		# The kubelet lets go of the volume a little after the pod is gone.
		exec_host "${node}" "for i in \$(seq 1 30); do \
			dmsetup remove --noudevsync ${dm_name} 2>/dev/null && break; sleep 2; done; \
			losetup -d ${loop_dev}; rm -f ${disk_image}" || true
	fi

	teardown_common "${node}" "${node_start_time:-}"
}
