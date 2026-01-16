#!/usr/bin/env bats
# Copyright 2024 IBM Corporation
# Copyright 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KBS="${KBS:-false}"
export test_key="aatest"
export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"
export RUNTIME_CLASS_NAME="kata-${KATA_HYPERVISOR}"
export AA_KBC="${AA_KBC:-cc_kbc}"

setup() {
	is_confidential_runtime_class || skip "Test not supported for ${KATA_HYPERVISOR}."

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	setup_common || die "setup_common failed"

	# install SNP measurement dependencies
	if [[ "${KATA_HYPERVISOR}" == *snp* ]]; then
		ensure_sev_snp_measure
		ensure_snphost

	fi

#	setup_unencrypted_confidential_pod

	if is_confidential_gpu_hardware; then
		POD_TEMPLATE_BASENAME="pod-attestable-gpu"
	else
		POD_TEMPLATE_BASENAME="pod-attestable"
	fi

	local pod_yaml_in="${pod_config_dir}/${POD_TEMPLATE_BASENAME}.yaml.in"
	export K8S_TEST_YAML="${pod_config_dir}/${POD_TEMPLATE_BASENAME}.yaml"

	# Substitute environment variables in the YAML template
	envsubst < "${pod_yaml_in}" > "${K8S_TEST_YAML}"

	# Schedule on a known node so that later it can print the system's logs for
	# debugging.
	set_node "$K8S_TEST_YAML" "$node"

	kbs_set_resource "default" "aa" "key" "$test_key"
	local CC_KBS_ADDR
	export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
	kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
	kernel_params_value="agent.guest_components_rest_api=resource"
	# Based on current config we still need to pass the agent.aa_kbc_params, but this might change
	# as the CDH/attestation-agent config gets updated
	if [ "${AA_KBC}" = "cc_kbc" ]; then
		kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
	fi
	set_metadata_annotation "${K8S_TEST_YAML}" \
		"${kernel_params_annotation}" \
		"${kernel_params_value}"
}

@test "Get CDH resource" {
	if is_confidential_gpu_hardware; then
		kbs_set_gpu0_resource_policy
	elif is_confidential_hardware; then
		kbs_set_default_policy
	else
		kbs_set_allow_all_resources
	fi

	kubectl apply -f "${K8S_TEST_YAML}"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	export pod_name=$(kubectl get pod -o wide | grep "aa-test-cc" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "${pod_name}"

	# Wait 5s for connecting with remote KBS
	sleep 5

	kubectl logs aa-test-cc
	cmd="kubectl logs aa-test-cc | grep -q ${test_key}"
	run bash -c "$cmd"
	[ "$status" -eq 0 ]

	if is_confidential_gpu_hardware; then
		cmd="kubectl logs aa-test-cc | grep -iq 'Confidential Compute GPUs Ready state:[[:space:]]*ready'"
		run bash -c "$cmd"
		[ "$status" -eq 0 ]
	fi
}

@test "Cannot get CDH resource when deny-all policy is set" {
	kbs_set_deny_all_resources
	kubectl apply -f "${K8S_TEST_YAML}"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	export pod_name=$(kubectl get pod -o wide | grep "aa-test-cc" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "${pod_name}"

	sleep 5

	kubectl logs aa-test-cc
	cmd="kubectl logs aa-test-cc | grep -q ${test_key}"
	run bash -c "$cmd"
	[ "$status" -eq 1 ]
}

# Negative test for getting a resource when the affirming policy is set
# (the AS policy must return an affirming trust vector), but no
# reference values are set.
#
# This can run on all platforms.
@test "Cannot get CDH resource when affirming policy is set without reference values" {

	# Require CPU0 to have affirming trust level.
	kbs_set_cpu0_resource_policy
	kubectl apply -f "${K8S_TEST_YAML}"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	export pod_name=$(kubectl get pod -o wide | grep "aa-test-cc" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "${pod_name}"

	sleep 5

	kubectl logs aa-test-cc
	cmd="kubectl logs aa-test-cc | grep -q ${test_key}"
	run bash -c "$cmd"
	[ "$status" -eq 1 ]
}

@test "Can get CDH resource when affirming policy is set with reference values" {

	[[ "${KATA_HYPERVISOR}" == *snp* ]] || skip "Test only supported with SNP"

	# set a policy that requires CPU0 to have affirming trust level
	kbs_set_cpu0_resource_policy

	# get measured artifacts from qemu command line of previous test
	log_line=$(sudo journalctl -r -x -t kata | grep -m 1 'launching.*qemu.*with:' || true)
	qemu_cmd=$(echo "$log_line" | sed 's/.*with: \[\(.*\)\]".*/\1/')
	[[ -n "$qemu_cmd" ]] || { echo "Could not find QEMU command line"; return 1; }

	kernel_path=$(echo "$qemu_cmd" | grep -oP -- '-kernel \K[^ ]+')
	initrd_path=$(echo "$qemu_cmd" | grep -oP -- '-initrd \K[^ ]+' || true)
	firmware_path=$(echo "$qemu_cmd" | grep -oP -- '-bios \K[^ ]+')
	vcpu_count=$(echo "$qemu_cmd" | grep -oP -- '-smp \K\d+')
	append=$(echo "$qemu_cmd" | sed -n 's/.*-append \(.*\) -bios.*/\1/p')

	measure_args=(
		--mode=snp
		--vcpus="${vcpu_count}"
		--vcpu-type=EPYC-v4
		--output-format=base64
		--ovmf="${firmware_path}"
		--kernel="${kernel_path}"
		--append="${append}"
	)
	if [[ -n "${initrd_path}" ]]; then
		measure_args+=(--initrd="${initrd_path}")
	fi
	launch_measurement=$(PATH="${PATH}:${HOME}/.local/bin" sev-snp-measure "${measure_args[@]}")

	# set launch measurement as reference value
	kbs_config_command set-sample-reference-value snp_launch_measurement "${launch_measurement}"

	# Get the reported firmware version(s) for this machine
	firmware=$(sudo snphost show tcb | grep -A 5 "Reported TCB")

	microcode_version=$(echo "$firmware" | grep -oP 'Microcode:\s*\K\d+')
	snp_version=$(echo "$firmware" | grep -oP 'SNP:\s*\K\d+')
	tee_version=$(echo "$firmware" | grep -oP 'TEE:\s*\K\d+')
	bootloader_version=$(echo "$firmware" | grep -oP 'Boot Loader:\s*\K\d+')

	kbs_config_command set-sample-reference-value --as-integer snp_bootloader "${bootloader_version}"
	kbs_config_command set-sample-reference-value --as-integer snp_microcode "${microcode_version}"
	kbs_config_command set-sample-reference-value --as-integer snp_snp_svn "${snp_version}"
	kbs_config_command set-sample-reference-value --as-integer snp_tee_svn "${tee_version}"

	kubectl apply -f "${K8S_TEST_YAML}"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	export pod_name=$(kubectl get pod -o wide | grep "aa-test-cc" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "${pod_name}"

	sleep 5

	kubectl logs aa-test-cc
	cmd="kubectl logs aa-test-cc | grep -q ${test_key}"
	run bash -c "$cmd"
	result=$status

	# Set the snp launch measurement to something invalid, such that future negative tests
	# running in the same context will work.
	kbs_config_command set-sample-reference-value snp_launch_measurement abcd

	[ "$result" -eq 0 ]
}


teardown() {
	is_confidential_runtime_class || skip "Test not supported for ${KATA_HYPERVISOR}."

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	confidential_teardown_common "${node}" "${node_start_time:-}"
}
