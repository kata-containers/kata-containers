#!/usr/bin/env bats
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Exercises the devkit debug guest extension through the agent debug console
# (kata-ctl exec <sandbox-id>) on the NVIDIA CPU runtime-rs class: with the
# kata-<shim>-devkit RuntimeClass, the console drops into the rich Ubuntu-based
# devkit shell overlaid on the read-only guest rootfs.

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

# The devkit debug console is a non-confidential debugging aid. For now it is
# validated only on the (non-confidential) NVIDIA CPU runtime-rs class; other
# hypervisors don't ship it in CI.
devkit_supported() {
	case "${KATA_HYPERVISOR}" in
		qemu-nvidia-cpu-runtime-rs) return 0 ;;
		*) return 1 ;;
	esac
}

devkit_runtimeclass() {
	echo "kata-${KATA_HYPERVISOR}-devkit"
}

# A probe the *guest* shell must evaluate: $((6*7)) only becomes "SHELL_OK=42"
# when a real shell runs it, so that string in the output is unambiguous proof
# that a debug console shell actually executed. The /real_root symlink is
# created only by devkit-init when the overlay is set up, so it tells the devkit
# chroot apart from the (also Ubuntu-based) NVIDIA base rootfs.
DEVKIT_PROBE='echo "SHELL_OK=$((6*7))"; . /etc/os-release 2>/dev/null; echo "GUEST_ID=${ID}"; test -L /real_root && echo "DEVKIT_OVERLAY=yes" || true'

check_and_skip() {
	if ! devkit_supported; then
		skip "devkit debug console not exercised for hypervisor: ${KATA_HYPERVISOR}"
	fi

	# The debug console client used here is kata-ctl, which ships only on x86_64
	# and aarch64. s390x and ppc64le cannot build it statically (it needs glibc),
	# so it is not installed there and this test cannot run.
	case "$(uname -m)" in
		x86_64 | aarch64) ;;
		*) skip "kata-ctl not shipped for $(uname -m); devkit debug console not exercised" ;;
	esac

	# The kata-<shim>-devkit RuntimeClass only exists when kata-deploy was
	# installed with both debug and devkit enabled. Skip (rather than fail)
	# where the extension was not deployed.
	if ! kubectl get runtimeclass "$(devkit_runtimeclass)" >/dev/null 2>&1; then
		skip "RuntimeClass $(devkit_runtimeclass) not found; devkit not deployed"
	fi
}

# Resolves the sandbox id into the global sandbox_id.
launch_pod() {
	local runtimeclass="$1"

	pod_config="$(new_pod_config "${nginx_image}" "${runtimeclass}")"
	set_node "${pod_config}" "${node}"
	yq -i ".metadata.name = \"${pod_name}\"" "${pod_config}"

	echo "Pod ${pod_config} (runtimeClass=${runtimeclass}):"
	cat "${pod_config}"

	kubectl create -f "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	# Ask the container runtime which sandbox belongs to this pod, rather than
	# looking for a shim in the node's process list: the process list is read
	# through the node debugger's chroot, where `ps -ef` intermittently dies
	# on a "fatal library error, lookup self", and it cannot tell this pod's
	# sandbox from any other one running on the node anyway.
	sandbox_id="$(get_pod_sandbox_id "${pod_name}")"
	[[ -n "${sandbox_id}" ]] || die "Failed to resolve the sandbox id of pod ${pod_name}"
	echo "sandbox id: ${sandbox_id}"
}

# Run DEVKIT_PROBE on the agent debug console for sandbox_id and echo what it
# printed. `kata-ctl exec ... -- <cmd>` waits for the console shell to be ready
# on its own, so no terminal has to be faked and no sleeps have to be guessed.
#
# DEVKIT_PROBE carries no single quotes, so it survives being single-quoted here
# for the remote shell.
run_debug_console() {
	local sandbox_id="$1"
	local remote="timeout 120 nsenter --mount=/proc/1/ns/mnt \
/opt/kata/bin/kata-ctl exec ${sandbox_id} -- sh -c '${DEVKIT_PROBE}' 2>&1"

	exec_host "${node}" "${remote}" || true
}

setup() {
	check_and_skip

	setup_common || die "setup_common failed"

	ensure_yq
	nginx_registry=$(get_from_kata_deps ".docker_images.nginx.registry")
	nginx_digest=$(get_from_kata_deps ".docker_images.nginx.digest")
	nginx_image="${nginx_registry}@${nginx_digest}"

	pod_name="devkit-debug-console"
}

@test "Debug console drops into the devkit shell" {
	launch_pod "$(devkit_runtimeclass)"

	local output
	output="$(run_debug_console "${sandbox_id}")"
	echo "debug console output:"
	echo "${output}"

	echo "${output}" | grep -q 'SHELL_OK=42' \
		|| die "devkit debug console did not provide a working shell"
	echo "${output}" | grep -q 'GUEST_ID=ubuntu' \
		|| die "devkit debug console did not report an Ubuntu guest"
	echo "${output}" | grep -q 'DEVKIT_OVERLAY=yes' \
		|| die "devkit debug console shell lacks /real_root; not the devkit overlay"
}

teardown() {
	check_and_skip

	kubectl describe "pod/${pod_name}" || true
	kubectl delete pod "${pod_name}" --ignore-not-found || true

	teardown_common "${node:-}" "${node_start_time:-}"
}
