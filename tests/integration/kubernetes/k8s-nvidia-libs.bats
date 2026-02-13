#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Check that a Kata GPU pod has the same set of NVIDIA libraries (by base name)
# as the in-repo reference list, ignoring driver version differences.
# Covers both TEE (SNP/TDX) and non-TEE; reference: nvidia-libs-reference.txt.
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-nvidia-gpu}"

TEE=false
if is_confidential_gpu_hardware; then
	TEE=true
fi
export TEE

export POD_NAME_NVIDIA_LIBS="nvidia-libs-check"
export REFERENCE_LIBS_FILE="${BATS_TEST_DIRNAME}/nvidia-libs-reference.txt"

POD_WAIT_TIMEOUT=${POD_WAIT_TIMEOUT:-300s}
export POD_WAIT_TIMEOUT

# Normalize library list: drop comments/blanks, basename, .so.<version> -> .so
normalize_lib_list() {
	grep -v '^[[:space:]]*$' | grep -v '^[[:space:]]*#' | awk -F/ '{print $NF}' | sed 's/\.so\..*/.so/' | sort -u
}

setup() {
	is_nvidia_gpu_platform || skip "Test only for NVIDIA GPU platform (KATA_HYPERVISOR=qemu-nvidia-gpu*)"

	[ -f "${REFERENCE_LIBS_FILE}" ] || skip "Reference file ${REFERENCE_LIBS_FILE} not found"

	setup_common || die "setup_common failed"

	pod_yaml_in="${pod_config_dir}/nvidia-libs-check.yaml.in"
	pod_yaml="${pod_config_dir}/nvidia-libs-check.yaml"
	envsubst < "${pod_yaml_in}" > "${pod_yaml}"

	if [ "${TEE}" = "true" ]; then
		kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
		kernel_params_value="nvrc.smi.srs=1"
		set_metadata_annotation "${pod_yaml}" \
			"${kernel_params_annotation}" \
			"${kernel_params_value}"
	fi

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	add_exec_to_policy_settings "${policy_settings_dir}" "uname" "-m"
	add_exec_to_policy_settings "${policy_settings_dir}" "ls"
	add_exec_to_policy_settings "${policy_settings_dir}" "sh"
	auto_generate_policy "${policy_settings_dir}" "${pod_yaml}"
}

@test "NVIDIA libraries presence matches reference" {
	kubectl apply -f "${pod_yaml}"
	kubectl wait --for=condition=Ready --timeout="${POD_WAIT_TIMEOUT}" pod "${POD_NAME_NVIDIA_LIBS}"

	ref_libs=$(cat "${REFERENCE_LIBS_FILE}")
	# Library path is arch-dependent: x86_64 -> /usr/lib/x86_64-linux-gnu, aarch64 -> /usr/lib/aarch64-linux-gnu
	arch=$(kubectl exec "${POD_NAME_NVIDIA_LIBS}" -- uname -m)
	lib_dir="/usr/lib/${arch}-linux-gnu"
	# libnvidia-* and libcuda.so
	pod_libs=$(kubectl exec "${POD_NAME_NVIDIA_LIBS}" -- sh -c "ls ${lib_dir}/libnvidia-* ${lib_dir}/libcuda.so* 2>/dev/null" || true)

	ref_normalized=$(echo "${ref_libs}" | normalize_lib_list)
	pod_normalized=$(echo "${pod_libs}" | normalize_lib_list)

	[ -n "${ref_normalized}" ] || skip "Reference file has no library entries (add normalized names, one per line)"
	[ -n "${pod_normalized}" ] || die "No libnvidia-* or libcuda.so libraries found in pod (check guest image and GPU passthrough)"

	# Compare: libraries in reference that are missing in pod
	missing=$(comm -23 <(echo "${ref_normalized}") <(echo "${pod_normalized}") || true)

	if [ -n "${missing}" ]; then
		echo "Libraries in reference but missing in pod:"
		echo "${missing}" | sed 's/^/  - /'
		false
	fi
}

teardown() {
	kubectl describe pod "${POD_NAME_NVIDIA_LIBS}" || true
	[ -n "${policy_settings_dir:-}" ] && delete_tmp_policy_settings_dir "${policy_settings_dir}"
	[ -f "${pod_yaml:-}" ] && kubectl delete -f "${pod_yaml}" --ignore-not-found=true
	print_node_journal_since_test_start "${node}" "${node_start_time:-}" "${BATS_TEST_COMPLETED:-}"
}
