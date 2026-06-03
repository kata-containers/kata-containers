#!/usr/bin/env bats
#
# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

pod_name="vsock-uds-forward-pod"
echo_pod_name="vsock-uds-forward-echo-pod"
vsock_fwd_port="15001"
guest_fwd_sock="/tmp/kata-vsock-fwd.sock"
uds_dir="/var/run/kata-vsock-uds-test"
uds_path="${uds_dir}/fwd.sock"
expected_response="kata-vsock-uds-forward-ok"
guest_request="test"
guest_client_hold=2
vsock_uds_forward_setting="${vsock_fwd_port}:${uds_path}"
guest_unix_client_cmd=(sh -c "( printf '%s\\n' '${guest_request}'; sleep ${guest_client_hold} ) | socat - UNIX-CONNECT:${guest_fwd_sock}")
guest_relay_wait_cmd=(test -S "${guest_fwd_sock}")

patch_vsock_uds_forward_dropin() {
	local local_dropin

	local_dropin="${BATS_FILE_TMPDIR}/99-vsock-uds-forward-test.toml"
	cat > "${local_dropin}" <<EOF
[runtime]
vsock_uds_forward = ["${vsock_uds_forward_setting}"]
EOF

	VSOCK_UDS_DROPIN_PATH="$(set_kata_runtime_config_dropin_file \
		"${node}" \
		"${local_dropin}")" || \
		die "failed to write Kata runtime config drop-in for ${KATA_HYPERVISOR}"
	export VSOCK_UDS_DROPIN_PATH

	echo "# Wrote drop-in ${VSOCK_UDS_DROPIN_PATH}"
}

restore_vsock_uds_forward_dropin() {
	remove_kata_runtime_config_dropin_file "${node}" "${VSOCK_UDS_DROPIN_PATH:-}" || true
}

start_uds_echo_server_pod() {
	local i

	exec_host "${node}" "mkdir -p '${uds_dir}'"
	kubectl create -f "${echo_yaml}"
	k8s_wait_pod_be_ready "${echo_pod_name}" "${wait_time}" || {
		kubectl describe pod "${echo_pod_name}" >&3
		kubectl logs "${echo_pod_name}" >&3 2>/dev/null || true
		die "UDS echo server pod did not become ready"
	}

	for i in $(seq 1 30); do
		if exec_host "${node}" "test -S '${uds_path}'"; then
			return 0
		fi
		sleep 1
	done

	kubectl logs "${echo_pod_name}" >&3 2>/dev/null || true
	die "UDS echo server did not create ${uds_path}"
}

create_and_wait_test_pod() {
	kubectl create -f "${yaml_file}"
	k8s_wait_pod_be_ready "${pod_name}" "${wait_time}" || {
		kubectl describe pod "${pod_name}" >&3
		kubectl logs "${pod_name}" >&3 2>/dev/null || true
		return 1
	}
}

wait_for_guest_relay_sock() {
	local i

	# Dual-listen socat creates the unix socket only after the shim connects on vsock.
	for i in $(seq 1 60); do
		if kubectl exec "${pod_name}" -- test -S "${guest_fwd_sock}" 2>/dev/null; then
			return 0
		fi
		sleep 1
	done

	die "guest relay socket ${guest_fwd_sock} not ready"
}

guest_unix_request() {
	kubectl exec "${pod_name}" -- "${guest_unix_client_cmd[@]}"
}

setup() {
	[[ "${KATA_HYPERVISOR}" == qemu* ]] || skip "vsock UDS forward requires QEMU (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
	is_runtime_rs && skip "vsock UDS forward requires the Go shim (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

	setup_common || die "setup_common failed"
	get_pod_config_dir

	yaml_file="${pod_config_dir}/pod-vsock-uds-forward.yaml"
	echo_yaml="${pod_config_dir}/pod-vsock-uds-forward-echo.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${policy_settings_dir}" "${guest_unix_client_cmd[@]}"
	add_exec_to_policy_settings "${policy_settings_dir}" sh socat printf sleep
	add_exec_to_policy_settings "${policy_settings_dir}" "${guest_relay_wait_cmd[@]}"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	patch_vsock_uds_forward_dropin

	start_uds_echo_server_pod
}

@test "guest unix request is forwarded to host UDS and returns response" {
	create_and_wait_test_pod

	wait_for_guest_relay_sock

	output="$(guest_unix_request)" || {
		kubectl logs "${echo_pod_name}" >&3 2>/dev/null || true
		die "guest unix request failed"
	}

	[[ "${output}" == *"${expected_response}"* ]] || {
		echo "# guest_unix_request output: ${output}" >&3
		kubectl logs "${echo_pod_name}" >&3 2>/dev/null || true
		false
	}
}

@test "vsock UDS forward pod shuts down cleanly after guest relay is ready" {
	create_and_wait_test_pod

	wait_for_guest_relay_sock
}

teardown() {
	[[ -z "${node:-}" ]] && return

	restore_vsock_uds_forward_dropin

	kubectl delete -f "${yaml_file}" --ignore-not-found=true
	kubectl delete -f "${echo_yaml}" --ignore-not-found=true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
