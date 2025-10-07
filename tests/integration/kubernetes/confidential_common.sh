#!/usr/bin/env bash
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

source "${BATS_TEST_DIRNAME}/tests_common.sh"
source "${BATS_TEST_DIRNAME}/../../common.bash"

load "${BATS_TEST_DIRNAME}/confidential_kbs.sh"

SUPPORTED_TEE_HYPERVISORS=("qemu-snp" "qemu-tdx" "qemu-se" "qemu-se-runtime-rs")
SUPPORTED_NON_TEE_HYPERVISORS=("qemu-coco-dev")

function setup_unencrypted_confidential_pod() {
	get_pod_config_dir

	export SSH_KEY_FILE="${pod_config_dir}/confidential/unencrypted/ssh/unencrypted"

	if [ -n "${GH_PR_NUMBER}" ]; then
		# Use correct address in pod yaml
		sed -i "s/-nightly/-${GH_PR_NUMBER}/" "${pod_config_dir}/pod-confidential-unencrypted.yaml"
	fi

	# Set permissions on private key file
	sudo chmod 600 "${SSH_KEY_FILE}"
}

# This function relies on `KATA_HYPERVISOR` being an environment variable
# and returns the remote command to be executed to that specific hypervisor
# in order to identify whether the workload is running on a TEE environment
function get_remote_command_per_hypervisor() {
	case "${KATA_HYPERVISOR}" in
		qemu-se*)
			echo "cd /sys/firmware/uv; cat prot_virt_guest | grep 1"
			;;
		qemu-snp)
			echo "dmesg | grep \"Memory Encryption Features active:.*SEV-SNP\""
			;;
		qemu-tdx)
			echo "cpuid | grep TDX_GUEST"
			;;
		*)
			echo ""
			;;
	esac
}

# This function verifies whether the input hypervisor supports confidential tests and
# relies on `KATA_HYPERVISOR` being an environment variable
function check_hypervisor_for_confidential_tests() {
	local kata_hypervisor="${1}"
	# This check must be done with "<SPACE>${KATA_HYPERVISOR}<SPACE>" to avoid
	# having substrings, like qemu, being matched with qemu-$something.
	if check_hypervisor_for_confidential_tests_tee_only "${kata_hypervisor}" ||\
	[[ " ${SUPPORTED_NON_TEE_HYPERVISORS[*]} " =~ " ${kata_hypervisor} " ]]; then
		return 0
	else
		return 1
	fi
}

# This function verifies whether the input hypervisor supports confidential tests and
# relies on `KATA_HYPERVISOR` being an environment variable
function check_hypervisor_for_confidential_tests_tee_only() {
	local kata_hypervisor="${1}"
	# This check must be done with "<SPACE>${KATA_HYPERVISOR}<SPACE>" to avoid
	# having substrings, like qemu, being matched with qemu-$something.
	if [[ " ${SUPPORTED_TEE_HYPERVISORS[*]} " =~ " ${kata_hypervisor} " ]]; then
		return 0
	fi

	return 1
}

# Common check for confidential tests.
function is_confidential_runtime_class() {
	if check_hypervisor_for_confidential_tests "${KATA_HYPERVISOR}"; then
		return 0
	fi

	return 1
}

# Common check for confidential hardware tests.
function is_confidential_hardware() {
	if check_hypervisor_for_confidential_tests_tee_only "${KATA_HYPERVISOR}"; then
		return 0
	fi

	return 1
}

function create_loop_device(){
	local loop_file="${1:-/tmp/trusted-image-storage.img}"
	local node="$(get_one_kata_node)"
	cleanup_loop_device "$loop_file"

	exec_host "$node" "dd if=/dev/zero of=$loop_file bs=1M count=2500"
	exec_host "$node" "losetup -fP $loop_file >/dev/null 2>&1"
	local device=$(exec_host "$node" losetup -j $loop_file | awk -F'[: ]' '{print $1}')

	echo $device
}

function cleanup_loop_device(){
	local loop_file="${1:-/tmp/trusted-image-storage.img}"
	local node="$(get_one_kata_node)"
	# Find all loop devices associated with $loop_file
	local existed_devices=$(exec_host "$node" losetup -j $loop_file | awk -F'[: ]' '{print $1}')

	if [ -n "$existed_devices" ]; then
		# Iterate over each found loop device and detach it
		for d in $existed_devices; do
			exec_host "$node" "losetup -d "$d" >/dev/null 2>&1"
		done
	fi

	exec_host "$node" "rm -f "$loop_file" >/dev/null 2>&1 || true"
}

# This function creates pod yaml. Parameters
# - $1: image reference
# - $2: image policy file. If given, `enable_signature_verification` will be set to true
# - $3: image registry auth.
# - $4: guest components procs parameter
# - $5: guest components rest api parameter
# - $6: node
function create_coco_pod_yaml() {
	image=$1
	image_policy=${2:-}
	image_registry_auth=${3:-}
	guest_components_procs=${4:-}
	guest_components_rest_api=${5:-}
	node=${6:-}

	local CC_KBS_ADDR
	export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)

	kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
	kernel_params_value=""

	if [ -n "$image_policy" ]; then
		kernel_params_value+=" agent.image_policy_file=${image_policy}"
		kernel_params_value+=" agent.enable_signature_verification=true"
	fi

	if [ -n "$image_registry_auth" ]; then
		kernel_params_value+=" agent.image_registry_auth=${image_registry_auth}"
	fi

	if [ -n "$guest_components_procs" ]; then
		kernel_params_value+=" agent.guest_components_procs=${guest_components_procs}"
	fi

	if [ -n "$guest_components_rest_api" ]; then
		kernel_params_value+=" agent.guest_components_rest_api=${guest_components_rest_api}"
	fi

	kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"

	# Note: this is not local as we use it in the caller test
	kata_pod="$(new_pod_config "$image" "kata-${KATA_HYPERVISOR}")"
	set_container_command "${kata_pod}" "0" "sleep" "30"

	# Set annotations
	set_metadata_annotation "${kata_pod}" \
		"io.containerd.cri.runtime-handler" \
		"kata-${KATA_HYPERVISOR}"
	set_metadata_annotation "${kata_pod}" \
		"${kernel_params_annotation}" \
		"${kernel_params_value}"

	add_allow_all_policy_to_yaml "${kata_pod}"

	if [ -n "$node" ]; then
		set_node "${kata_pod}" "$node"
	fi
}

# This function creates pod yaml. Parameters
# - $1: image reference
# - $2: annotation `io.katacontainers.config.hypervisor.kernel_params`
# - $3: annotation `io.katacontainers.config.hypervisor.cc_init_data`
# - $4: node
function create_coco_pod_yaml_with_annotations() {
	image=$1
	kernel_params_annotation_value=${2:-}
	cc_initdata_annotation_value=${3:-}
	node=${4:-}

	kernel_params_annotation_key="io.katacontainers.config.hypervisor.kernel_params"
	cc_initdata_annotation_key="io.katacontainers.config.hypervisor.cc_init_data"

	# Note: this is not local as we use it in the caller test
	kata_pod="$(new_pod_config "$image" "kata-${KATA_HYPERVISOR}")"
	set_container_command "${kata_pod}" "0" "sleep" "30"

	# Set annotations
	set_metadata_annotation "${kata_pod}" \
		"io.containerd.cri.runtime-handler" \
		"kata-${KATA_HYPERVISOR}"
	set_metadata_annotation "${kata_pod}" \
		"${kernel_params_annotation_key}" \
		"${kernel_params_annotation_value}"
	set_metadata_annotation "${kata_pod}" \
		"${cc_initdata_annotation_key}" \
		"${cc_initdata_annotation_value}"

	if [ -n "$node" ]; then
		set_node "${kata_pod}" "$node"
	fi
}

function get_initdata_with_cdh_image_section() {
	CDH_IMAGE_SECTION=${1:-""}

	CC_KBS_ADDRESS=$(kbs_k8s_svc_http_addr)

	 initdata_annotation=$(gzip -c << EOF | base64 -w0
version = "0.1.0"
algorithm = "sha256"
[data]
"aa.toml" = '''
[token_configs]
[token_configs.kbs]
url = "${CC_KBS_ADDRESS}"
'''

"cdh.toml" = '''
[kbc]
name = "cc_kbc"
url = "${CC_KBS_ADDRESS}"

${CDH_IMAGE_SECTION}
'''

"policy.rego" = '''
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

package agent_policy

default AddARPNeighborsRequest := true
default AddSwapRequest := true
default CloseStdinRequest := true
default CopyFileRequest := true
default CreateContainerRequest := true
default CreateSandboxRequest := true
default DestroySandboxRequest := true
default ExecProcessRequest := true
default GetMetricsRequest := true
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default ListInterfacesRequest := true
default ListRoutesRequest := true
default MemHotplugByProbeRequest := true
default OnlineCPUMemRequest := true
default PauseContainerRequest := true
default PullImageRequest := true
default ReadStreamRequest := true
default RemoveContainerRequest := true
default RemoveStaleVirtiofsShareMountsRequest := true
default ReseedRandomDevRequest := true
default ResumeContainerRequest := true
default SetGuestDateTimeRequest := true
default SetPolicyRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StartTracingRequest := true
default StatsContainerRequest := true
default StopTracingRequest := true
default TtyWinResizeRequest := true
default UpdateContainerRequest := true
default UpdateEphemeralMountsRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := true
'''
EOF
    )
    echo "${initdata_annotation}"
}

confidential_teardown_common() {
	local node="$1"
	local node_start_time="$2"

	# Run common teardown
	teardown_common "${node}" ${node_start_time}

	# Also try and print the kbs logs on failure
	if [[ -n "${node_start_time}" && -z "${BATS_TEST_COMPLETED}" ]]; then
		kbs_k8s_print_logs "${node_start_time}"
	fi
}
