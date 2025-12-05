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

	setup_common
	get_pod_config_dir

	# install SNP measurement dependencies
	if [[ "${KATA_HYPERVISOR}" == *snp* ]]; then
		# shellcheck disable=SC1091  # Sourcing virtual environment activation script
		source "${HOME}"/.cicd/venv/bin/activate

		pip install --upgrade pip
		pip install sev-snp-measure
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

	setup_cdi_override_for_nvidia_gpu_snp
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

# this can run on all platforms
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
	cmd="kubectl logs aa-test-cc | grep -q aatest"
	run $cmd
	[ "$status" -eq 1 ]
}

# only on sample, snp, snp-gpu
@test "Can get CDH resource when affirming policy is set with reference values" {

	[[ "${KATA_HYPERVISOR}" == *snp* ]] || skip "Test only supported with SNP"

	# set a policy that requires CPU0 to have affirming trust level
	kbs_set_cpu0_resource_policy

	# get measured artifacts from qemu command line of previous test
	log_line=$(exec_host "${node}" "journalctl -r -x -t kata --since '${node_start_time}' | grep -m 1 'launching.*qemu.*with:'" || true)
	qemu_cmd=$(echo "$log_line" | sed 's/.*with: \[\(.*\)\]".*/\1/')

	echo "$qemu_cmd"

	kernel_path=$(echo "$qemu_cmd" | grep -oP -- '-kernel \K[^ ]+')
	initrd_path=$(echo "$qemu_cmd" | grep -oP -- '-initrd \K[^ ]+')
	firmware_path=$(echo "$qemu_cmd" | grep -oP -- '-bios \K[^ ]+')
	append=$(echo "$qemu_cmd" | sed -n 's/.*-append \(.*\) -bios.*/\1/p')

	# calculate the expected launch measurement
	vcpu_sig=$(cpuid -1 --leaf 0x1 --raw | cut -s -f2 -d= | cut -f1 -d" ")

	launch_measurement=$(PATH="${PATH}:${HOME}/.local/bin" sev-snp-measure \
		--mode=snp \
		--vcpus=1 \
		--vcpu-sig="${vcpu_sig}" \
		--output-format=base64 \
		--ovmf="${firmware_path}" \
		--kernel="${kernel_path}" \
		--initrd="${initrd_path}" \
		--append="${append}" \
	)

	# set reference values for Trustee
	kbs_config_command "set-sample-reference-value snp-launch-measurement ${launch_measurement}"

	# firmware versions
	kbs_config_command "set-sample-reference-value --as-integer snp_bootloader 10"
	kbs_config_command "set-sample-reference-value --as-integer snp_microcode 84"
	kbs_config_command "set-sample-reference-value --as-integer snp_snp_svn 25"
	kbs_config_command "set-sample-reference-value --as-integer snp_tee_svn 0"

	kubectl apply -f "${K8S_TEST_YAML}"

	# Retrieve pod name, wait for it to come up, retrieve pod ip
	export pod_name=$(kubectl get pod -o wide | grep "aa-test-cc" | awk '{print $1;}')

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "${pod_name}"

	sleep 5

	kubectl logs aa-test-cc
	kubectl logs aa-test-cc | grep -q "aatest"
}


teardown() {
	is_confidential_runtime_class || skip "Test not supported for ${KATA_HYPERVISOR}."

	if [ "${KBS}" = "false" ]; then
		skip "Test skipped as KBS not setup"
	fi

	teardown_cdi_override_for_nvidia_gpu_snp

	confidential_teardown_common "${node}" "${node_start_time:-}"
}
