#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# When the guest stops answering, the Go runtime's monitor marks the agent
# dead and tears the sandbox down on its own. A hot-plugged virtio-blk disk can
# only be unplugged with the guest's help, so that teardown must not wait for
# the guest to let go of it: the VM has to be stopped, and nothing of the
# sandbox, QEMU included, may be left behind.
#
# The guest is frozen by pausing its vCPUs over a second QMP monitor. QEMU
# itself keeps running, so the runtime's own QMP connection keeps answering and
# only the agent goes quiet. Stopping the whole QEMU process with SIGSTOP would
# not do: the runtime checks the hypervisor before the agent, that check has no
# deadline, and the agent would never be marked dead.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

# Run QMP commands argv[2:] on the monitor socket argv[1] and print what the
# last one returns. Events are skipped; an error ends the script.
read -r -d '' qmp_client <<'EOF' || true
import json
import socket
import sys

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.settimeout(10)
sock.connect(sys.argv[1])
stream = sock.makefile("rw")


def reply():
    while True:
        line = stream.readline()
        if not line:
            sys.exit("QMP connection closed")
        msg = json.loads(line)
        if "error" in msg:
            sys.exit("QMP error: %s" % msg["error"])
        if "event" not in msg:
            return msg.get("return")


def execute(command):
    stream.write(json.dumps({"execute": command}) + "\n")
    stream.flush()
    return reply()


reply()
execute("qmp_capabilities")
result = None
for command in sys.argv[2:]:
    result = execute(command)
print(json.dumps(result))
EOF

qmp() {
	local socket="$1"
	shift

	exec_host "${node}" "echo $(base64 -w0 <<<"${qmp_client}") | base64 -d | python3 - ${socket} $*"
}

# Print how many descriptors process $1 holds on the volume's loop device. The
# kubelet hands the device over under a path of its own, so compare device
# numbers rather than names.
loop_dev_descriptors() {
	local pid="$1"

	exec_host "${node}" "want=\$(stat -c '%t:%T' ${loop_dev}); n=0; \
		for fd in /proc/${pid}/fd/*; do \
			[ \"\$(stat -L -c '%t:%T' \"\${fd}\" 2>/dev/null)\" = \"\${want}\" ] && n=\$((n + 1)); \
		done; echo \${n}"
}

# Succeed while the sandbox's QEMU runs and fail once it is gone. A query that
# did not get an answer from the node must not pass for an exited QEMU, so it
# ends the test instead.
qemu_alive() {
	local state

	state="$(exec_host "${node}" "pgrep -f '[q]emu.*${sandbox_id}' >/dev/null; \
		case \$? in 0) echo alive ;; 1) echo gone ;; *) echo 'pgrep failed' ;; esac")"
	case "${state}" in
		alive) return 0 ;;
		gone) return 1 ;;
		*) die "Could not tell whether QEMU of sandbox ${sandbox_id} runs on ${node}: '${state}'" ;;
	esac
}

setup() {
	[[ "${KATA_HYPERVISOR}" == "qemu" ]] || skip "dead agent teardown is tested for QEMU on the Go runtime only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

	setup_common || die "setup_common failed"

	pod_name="pod-block-pv"
	volume_name="block-loop-pv"
	volume_claim="block-loop-pvc"
	vol_capacity="500M"

	has_python="$(exec_host "${node}" "command -v python3 >/dev/null && echo yes || echo no")"
	[[ "${has_python}" =~ ^(yes|no)$ ]] || die "Could not run commands on ${node}: '${has_python}'"
	[[ "${has_python}" == "yes" ]] || skip "python3 is needed on ${node} to talk to QMP"

	# The extra monitor is only created with hypervisor debug on. virtio-scsi,
	# the default, unplugs a disk without asking the guest, so it would not
	# wait on a frozen one; virtio-blk does.
	local_dropin="$(mktemp --tmpdir 60-dead-agent-block-teardown.XXXXX.toml)"
	cat > "${local_dropin}" <<-EOF
		[hypervisor.qemu]
		enable_debug = true
		extra_monitor_socket = "qmp"
		block_device_driver = "virtio-blk"
	EOF
	runtime_config_dropin="$(set_kata_runtime_config_dropin_file "${node}" "${local_dropin}")" || \
		die "failed to write the drop-in for ${KATA_HYPERVISOR}"

	disk_image="$(exec_host "${node}" mktemp --tmpdir disk.XXXXXX.img)"
	exec_host "${node}" truncate "${disk_image}" --size "${vol_capacity}"
	loop_dev="$(exec_host "${node}" losetup -f --show "${disk_image}")"
	[[ "${loop_dev}" == /dev/loop* ]] || die "Could not set up a loop device on ${node}: '${loop_dev}'"

	kubectl create -f volume/local-storage.yaml
	pv_config="$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)"
	sed -e "s|LOOP_DEVICE|${loop_dev}|" \
		-e "s|HOSTNAME|${node}|" \
		-e "s|CAPACITY|${vol_capacity}|" \
		volume/block-loop-pv.yaml > "${pv_config}"
	kubectl create -f "${pv_config}"
	pvc_config="$(mktemp --tmpdir block_persistent_vol.XXXXX.yaml)"
	sed -e "s|CAPACITY|${vol_capacity}|" volume/block-loop-pvc.yaml > "${pvc_config}"
	kubectl create -f "${pvc_config}"

	pod_config="$(mktemp --tmpdir pod-block-pv.XXXXX.yaml)"
	sed -e "s|DEVICE_PATH|/dev/xda|" "${pod_config_dir}/${pod_name}.yaml" > "${pod_config}"
	# Without this, the kubelet replaces the torn down sandbox, and the
	# replacement's QEMU would be taken for the one that has to go.
	yq -i '.spec.restartPolicy = "Never"' "${pod_config}"
	auto_generate_policy "${pod_config_dir}" "${pod_config}"
}

@test "A sandbox whose agent died is torn down without waiting for its hot-plugged disk" {
	local qemu_pid
	local monitor
	local status
	local descriptors

	kubectl create -f "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	sandbox_id="$(get_pod_sandbox_id "${pod_name}")"
	[[ -n "${sandbox_id}" ]] || die "No sandbox ID found for pod ${pod_name}"
	qemu_pid="$(get_vmm_pid_for_pod "${pod_name}")"
	[[ "${qemu_pid}" =~ ^[0-9]+$ ]] || die "No single QEMU found for pod ${pod_name}: '${qemu_pid}'"

	# Without these the test would pass without testing anything: the disk
	# has to be in QEMU, and the drop-in that makes it virtio-blk has to have
	# been read, which the extra monitor shows.
	descriptors="$(loop_dev_descriptors "${qemu_pid}")"
	[[ "${descriptors}" -gt 0 ]] || die "QEMU holds no descriptor on ${loop_dev}, the disk was not hot-plugged"
	monitor="/run/vc/vm/${sandbox_id}/extra-monitor.sock"
	[[ "$(exec_host "${node}" "test -S '${monitor}' && echo present")" == "present" ]] || \
		die "No extra QMP monitor at ${monitor}, the drop-in ${runtime_config_dropin} was not applied"
	# Read by wait_for_sandbox_cleanup().
	# shellcheck disable=SC2034
	sandbox_cleanup_paths=("/run/vc/sbs/${sandbox_id}" "/run/vc/vm/${sandbox_id}")
	[[ "$(exec_host "${node}" "test -d '/run/vc/sbs/${sandbox_id}' && echo present")" == "present" ]] || \
		die "No persisted state for sandbox ${sandbox_id}"

	status="$(qmp "${monitor}" stop query-status)"
	[[ "${status}" == *'"status": "paused"'* ]] || die "The guest was not paused: '${status}'"

	# The agent check times out after 30 seconds and runs every 5. Once the
	# agent is dead the VM must go without any help from the guest. Without
	# that, the teardown waits on the disk's unplug, and QEMU stays paused
	# for good.
	waitForProcess 120 5 "! qemu_alive" || \
		die "QEMU of sandbox ${sandbox_id} is still there 120s after its guest was paused"

	kubectl delete pod "${pod_name}" --wait=true --timeout="${timeout}"
	wait_for_sandbox_cleanup "${node}" "${sandbox_id}"
}

teardown() {
	[[ "${KATA_HYPERVISOR}" == "qemu" ]] || skip "dead agent teardown is tested for QEMU on the Go runtime only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

	local dropin_removed=true

	# A runtime that hangs keeps a paused QEMU, which would hold the volume
	# and the node's memory past this test. Kill it without asking first: a
	# query that fails would leave it running.
	if [[ -n "${sandbox_id:-}" ]]; then
		exec_host "${node}" "pkill -KILL -f '[q]emu.*${sandbox_id}' || true" || true
	fi

	# A drop-in left behind would change every later pod on the node, so it
	# goes first, before anything that can fail or hang. The runtime has read
	# it already.
	remove_kata_runtime_config_dropin_file "${node}" "${runtime_config_dropin:-}" || dropin_removed=false

	kubectl describe "pod/${pod_name}" || true

	kubectl delete pod "${pod_name}" --ignore-not-found=true --wait=true --timeout="${timeout}" || true
	kubectl delete pvc "${volume_claim}" --ignore-not-found=true --timeout="${timeout}" || true
	kubectl delete pv "${volume_name}" --ignore-not-found=true --timeout="${timeout}" || true
	kubectl delete storageclass local-storage --ignore-not-found=true --timeout="${timeout}" || true
	rm -f "${pv_config:-}" "${pvc_config:-}" "${pod_config:-}" "${local_dropin:-}"

	if [[ -n "${loop_dev:-}" ]]; then
		exec_host "${node}" "losetup -d ${loop_dev}; rm -f ${disk_image}" || true
	fi

	teardown_common "${node}" "${node_start_time:-}"

	[[ "${dropin_removed}" == "true" ]] || \
		die "Could not remove the drop-in ${runtime_config_dropin}, later pods on ${node} will use it"
}
