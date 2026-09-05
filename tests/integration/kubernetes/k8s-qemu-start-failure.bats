#!/usr/bin/env bats
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Verify that runtime-rs kills and reaps QEMU when setup fails after spawn.

# shellcheck disable=SC2154 # Bats and sourced helpers define these variables.
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	if ! is_runtime_rs || [[ "${KATA_HYPERVISOR}" != qemu*-runtime-rs ]]; then
		skip "QEMU post-spawn cleanup applies only to runtime-rs QEMU"
	fi
	if [[ "$(uname -m)" == "s390x" ]]; then
		skip "s390x supports the virtio-mem setup used to force this failure"
	fi

	setup_common || die "setup_common failed"

	pod_name="qemu-start-failure"
	pod_config="${BATS_TEST_TMPDIR}/pod-qemu-start-failure.yaml"
	cp "${pod_config_dir}/pod-qemu-start-failure.yaml" "${pod_config}"
	yq -i ".spec.runtimeClassName = \"$(get_test_runtime_class)\"" "${pod_config}"
	set_node "${pod_config}" "${node}"
	auto_generate_policy "${pod_config_dir}" "${pod_config}"

	local runtime_config_dropin_file="${BATS_TEST_TMPDIR}/99-k8s-qemu-start-failure.toml"
	cat > "${runtime_config_dropin_file}" <<EOF
[hypervisor.qemu]
default_memory = 512
default_maxmemory = 1024
memory_slots = 1
enable_virtio_mem = true
EOF

	dropin_path="$(set_kata_runtime_config_dropin_file \
		"${node}" \
		"${runtime_config_dropin_file}")" || \
		die "Failed to install QEMU start-failure config drop-in on ${node}"
}

@test "QEMU is reaped when post-spawn setup fails" {
	retry_kubectl_apply "${pod_config}"

	local deadline=$((SECONDS + 120))
	local logs=""
	while (( SECONDS < deadline )); do
		logs="$(exec_host "${node}" journalctl -x -t kata \
			--since '"'"${node_start_time}"'"' --no-pager 2>/dev/null || true)"
		if grep -q "QEMU startup failed:.*virtio-mem supports" <<< "${logs}"; then
			break
		fi
		sleep 2
	done

	if ! grep -q "QEMU startup failed:.*virtio-mem supports" <<< "${logs}"; then
		kubectl describe pod "${pod_name}" >&3 || true
		echo "${logs}" >&3
		return 1
	fi

	mapfile -t qemu_pids < <(
		sed -nE \
			's/.*qemu process started \(pid ([0-9]+), sandbox [^)]+\).*/\1/p' \
			<<< "${logs}" | sort -u
	)
	[[ "${#qemu_pids[@]}" -gt 0 ]]

	# Stop kubelet from creating another failed sandbox while checking every
	# QEMU process spawned for this test.
	kubectl delete pod "${pod_name}" --ignore-not-found=true --wait=false

	local pid
	for pid in "${qemu_pids[@]}"; do
		local attempt
		for ((attempt = 0; attempt < 30; attempt++)); do
			if ! exec_host "${node}" "test -e /proc/${pid}" >/dev/null 2>&1; then
				break
			fi
			sleep 1
		done
		! exec_host "${node}" "test -e /proc/${pid}" >/dev/null 2>&1
	done
}

teardown() {
	[[ -n "${node:-}" ]] || return 0

	kubectl delete pod "${pod_name:-qemu-start-failure}" \
		--ignore-not-found=true --wait=false 2>/dev/null || true
	remove_kata_runtime_config_dropin_file \
		"${node}" \
		"${dropin_path:-}" || true
	teardown_common "${node}" "${node_start_time:-}"
}
