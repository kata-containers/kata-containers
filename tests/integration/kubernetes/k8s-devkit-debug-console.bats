#!/usr/bin/env bats
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Exercises the devkit debug guest extension through the agent debug console
# (kata-ctl exec <sandbox-id>) on the NVIDIA CPU runtime-rs class:
#
#   * With the kata-<shim>-devkit RuntimeClass, the console drops into the rich
#     Alpine-based devkit shell overlaid on the read-only guest rootfs.
#   * Without devkit (the base RuntimeClass), the shell-less NVIDIA base cannot
#     spawn a console shell at all.

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
# when a real shell runs it. The literal command text, even if echoed back by
# the PTY, never contains the expanded value - so "SHELL_OK=42" in the output is
# unambiguous proof that a debug console shell actually executed. GUEST_ID and
# apk then tell devkit (Alpine overlay) apart from the base rootfs.
DEVKIT_PROBE='echo "SHELL_OK=$((6*7))"; . /etc/os-release 2>/dev/null; echo "GUEST_ID=${ID}"; command -v apk >/dev/null 2>&1 && apk --version || true'

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

	sandbox_id="$(get_node_kata_sandbox_id "${node}")"
	[[ -n "${sandbox_id}" ]] || die "Failed to resolve kata sandbox id on node ${node}"
	echo "sandbox id: ${sandbox_id}"
}

# Drive the interactive agent debug console for sandbox_id with DEVKIT_PROBE and
# echo the combined output.
#
# The console is an interactive PTY, so a bare pipe races the guest login shell
# startup and loses the input. Drive it with a real terminal via `script`
# (util-linux), feeding commands through a FIFO whose writer stays open long
# enough for the shell to be ready before input arrives and to flush output
# before we send `exit`.
run_debug_console() {
	local sandbox_id="$1"
	local remote="
fifo=\$(mktemp -u); mkfifo \"\${fifo}\"
( sleep 2; printf '%s\\n' '${DEVKIT_PROBE}'; sleep 3; printf 'exit\\n'; sleep 1 ) > \"\${fifo}\" &
timeout 120 script -qec \"nsenter --mount=/proc/1/ns/mnt /opt/kata/bin/kata-ctl exec ${sandbox_id}\" /dev/null < \"\${fifo}\" 2>&1
rm -f \"\${fifo}\"
"
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
	echo "${output}" | grep -q 'GUEST_ID=alpine' \
		|| die "devkit debug console did not report an Alpine guest"
	echo "${output}" | grep -q 'apk-tools' \
		|| die "devkit debug console shell lacks apk; not the devkit overlay"
}

@test "Without devkit, the NVIDIA base debug console has no usable shell" {
	pod_name="devkit-debug-console-negative"

	# The base RuntimeClass never carries the devkit extension, regardless of
	# whether devkit is deployed on the node.
	launch_pod "kata-${KATA_HYPERVISOR}"

	local output
	output="$(run_debug_console "${sandbox_id}")"
	echo "debug console output:"
	echo "${output}"

	# The NVIDIA base rootfs is shell-less (NVRC init, no /bin/bash|/bin/sh), so
	# even though debug mode enables the console the agent cannot stand up a
	# usable shell: kata-ctl gets its vsock connection reset (ECONNRESET) and
	# exits non-zero, so the probe never executes.
	if echo "${output}" | grep -q 'SHELL_OK=42'; then
		die "NVIDIA base rootfs unexpectedly provided a debug console shell"
	fi
}

teardown() {
	check_and_skip

	kubectl describe "pod/${pod_name}" || true
	kubectl delete pod "${pod_name}" --ignore-not-found || true

	teardown_common "${node:-}" "${node_start_time:-}"
}
