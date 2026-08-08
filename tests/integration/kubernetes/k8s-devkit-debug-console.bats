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
# when a real shell runs it. The literal command text, even if echoed back by
# the PTY, never contains the expanded value - so "SHELL_OK=42" in the output is
# unambiguous proof that a debug console shell actually executed. The /real_root
# symlink is created only by devkit-init when the overlay is set up, so it tells
# the devkit chroot apart from the (also Ubuntu-based) NVIDIA base rootfs.
#
# It also reports the PATH the shell runs with, plus one EXT_BIN line per tool
# directory the mounted extensions ship, which assert_extension_dirs_in_path()
# then cross-checks on this side.
#
# One complete command per element, joined by newlines below: the guest reads
# these from a PTY, so nothing is spread over continuation lines. Single quotes
# cannot appear either - run_debug_console() feeds the result through a
# single-quoted printf on the node.
DEVKIT_PROBE_COMMANDS=(
	'echo "SHELL_OK=$((6*7))"'
	'. /etc/os-release 2>/dev/null; echo "GUEST_ID=${ID}"'
	'test -L /real_root && echo "DEVKIT_OVERLAY=yes" || true'
	'echo "DEVKIT_PATH=${PATH}"'
	# Symlinked directories are skipped for the same reason devkit-init does
	# not list them (the GPU extension's usr/bin -> ../bin).
	'for d in /real_root/run/kata-extensions/*/bin; do test -d "${d}" && ! test -L "${d}" && echo "EXT_BIN=${d#/real_root}"; done'
)
DEVKIT_PROBE="$(printf '%s\n' "${DEVKIT_PROBE_COMMANDS[@]}")"

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

	sandbox_id="$(get_pod_sandbox_id "${pod_name}")"
	[[ -n "${sandbox_id}" ]] || die "Failed to resolve the sandbox id of pod ${pod_name}"
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

# Every tool directory the probe found under /run/kata-extensions must be in the
# PATH the devkit shell runs with, under the name the *guest* root uses: that is
# what `chroot /real_root <tool>` resolves against, so without it an extension's
# tools (e.g. the GPU extension's nvidia-smi) are only reachable by full path.
#
# A reported value is told apart from the probe text the PTY echoes back by
# where it sits and what it holds: at the start of a line, with a path for a
# value, while the echo carries `${PATH}` and `${d#/real_root}` unexpanded and
# behind a shell prompt.
#
# Passes vacuously where the devkit is the only extension cold-plugged, which is
# the case on the CPU-only class this test runs on today.
assert_extension_dirs_in_path() {
	local output="$1"
	local clean guest_path ext_dir missing=""

	# Readline brackets every line it accepts in paste-mode escapes, and the
	# closing one is written just before the command's output: on a PTY the
	# values only begin a line once the control sequences are dropped.
	clean="$(printf '%s\n' "${output}" | tr -d '\r' | sed $'s/\033\\[[0-9;?]*[A-Za-z]//g')"

	guest_path="$(printf '%s\n' "${clean}" | sed -n 's|^DEVKIT_PATH=\(/.*\)$|\1|p' | head -1)"
	[[ -n "${guest_path}" ]] || die "devkit debug console did not report its PATH"

	# The devkit's own bin/ is the chroot's root, not something PATH should
	# point at: its dynamic binaries cannot run on the guest base anyway.
	for ext_dir in $(printf '%s\n' "${clean}" | sed -n 's|^EXT_BIN=\(/.*\)$|\1|p'); do
		[[ "${ext_dir}" == */devkit/bin ]] && continue
		[[ ":${guest_path}:" == *":${ext_dir}:"* ]] && continue
		missing+=" ${ext_dir}"
	done

	[[ -z "${missing}" ]] || die "devkit shell PATH lacks extension tools at:${missing} (PATH=${guest_path})"
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

	assert_extension_dirs_in_path "${output}"
}

teardown() {
	check_and_skip

	kubectl describe "pod/${pod_name}" || true
	kubectl delete pod "${pod_name}" --ignore-not-found || true

	teardown_common "${node:-}" "${node_start_time:-}"
}
