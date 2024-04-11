#!/usr/bin/env bash
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

source "${BATS_TEST_DIRNAME}/tests_common.sh"
source "${BATS_TEST_DIRNAME}/../../common.bash"

SUPPORTED_TEE_HYPERVISORS=("qemu-sev" "qemu-snp" "qemu-tdx" "qemu-se")
SUPPORTED_NON_TEE_HYPERVISORS=("qemu")

function setup_unencrypted_confidential_pod() {
	get_pod_config_dir

	export SSH_KEY_FILE="${pod_config_dir}/confidential/unencrypted/ssh/unencrypted"

	if [ -n "${PR_NUMBER}" ]; then
		# Use correct address in pod yaml
		sed -i "s/-nightly/-${PR_NUMBER}/" "${pod_config_dir}/pod-confidential-unencrypted.yaml"
	fi

	# Set permissions on private key file
	sudo chmod 600 "${SSH_KEY_FILE}"
}

# This function relies on `KATA_HYPERVISOR` being an environment variable
# and returns the remote command to be executed to that specific hypervisor
# in order to identify whether the workload is running on a TEE environment
function get_remote_command_per_hypervisor() {
	declare -A REMOTE_COMMAND_PER_HYPERVISOR
	REMOTE_COMMAND_PER_HYPERVISOR[qemu-sev]="dmesg | grep \"Memory Encryption Features active:.*\(SEV$\|SEV \)\""
	REMOTE_COMMAND_PER_HYPERVISOR[qemu-snp]="dmesg | grep \"Memory Encryption Features active:.*SEV-SNP\""
	REMOTE_COMMAND_PER_HYPERVISOR[qemu-tdx]="cpuid | grep TDX_GUEST"
	REMOTE_COMMAND_PER_HYPERVISOR[qemu-se]="cd /sys/firmware/uv; cat prot_virt_guest | grep 1"

	echo "${REMOTE_COMMAND_PER_HYPERVISOR[${KATA_HYPERVISOR}]}"
}

# This function verifies whether the input hypervisor supports confidential tests and 
# relies on `KATA_HYPERVISOR` being an environment variable
function check_hypervisor_for_confidential_tests() {
	local kata_hypervisor="${1}"
	# This check must be done with "<SPACE>${KATA_HYPERVISOR}<SPACE>" to avoid
	# having substrings, like qemu, being matched with qemu-$something.
    if [[ " ${SUPPORTED_TEE_HYPERVISORS[*]} " =~ " ${kata_hypervisor} " ]] ||\
       [[ " ${SUPPORTED_NON_TEE_HYPERVISORS[*]} " =~ " ${kata_hypervisor} " ]]; then        
        return 0
    else
        return 1
    fi
}

function reset_k8s_images(){
	# we are encountering the issue (https://github.com/kata-containers/kata-containers/issues/8407)
	# with containerd on CI is likely due to the content digest being missing from the content store,
	# which can happen when switching between different snapshotters.
	# To help sort it out, we now clean up related images in k8s.io namespace.

	# remove related images in k8s.io namespace
	test_images_to_remove=(
		"registry.k8s.io/pause"
		"quay.io/sjenning/nginx"
		"quay.io/prometheus/busybox"
		"quay.io/confidential-containers/test-images"
	)

	ctr_args=""
	if [ "${KUBERNETES}" = "k3s" ]; then
		ctr_args="--address /run/k3s/containerd/containerd.sock "
	fi
	ctr_args+="--namespace k8s.io"
	ctr_command="sudo -E ctr ${ctr_args}"
	for related_image in "${test_images_to_remove[@]}"; do
		# We need to delete related image
		image_list=($(${ctr_command} i ls -q |grep "$related_image" |awk '{print $1}'))
		if [ "${#image_list[@]}" -gt 0 ]; then
			echo "image_list: ${image_list[@]}"
			for image in "${image_list[@]}"; do
				echo "image: $image"
				${ctr_command} i remove "$image"
			done
		fi
		
		# We need to delete related content of image
		IFS="/" read -ra parts <<< "$related_image"; 
		repository="${parts[0]}";     
		image_name="${parts[1]}";
		formatted_image="${parts[0]}=${parts[-1]}"
		image_contents=($(${ctr_command} content ls | grep "${formatted_image}" | awk '{print $1}'))
		if [ "${#image_contents[@]}" -gt 0 ]; then
			echo "image_contents: ${image_contents[@]}"
			for content in $image_contents; do
				echo "content: $content"
				${ctr_command} content rm "$content"
			done
		fi
	done
}

# Common setup for confidential tests.
function confidential_setup() {
	ensure_yq
	if ! check_hypervisor_for_confidential_tests "${KATA_HYPERVISOR}"; then
        return 1
    elif [[ " ${SUPPORTED_NON_TEE_HYPERVISORS[*]} " =~ " ${KATA_HYPERVISOR} " ]]; then
        info "Need to apply image annotations"
    fi
	reset_k8s_images
}
