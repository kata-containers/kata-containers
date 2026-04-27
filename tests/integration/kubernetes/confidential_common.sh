#!/usr/bin/env bash
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
# Copyright 2026 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# shellcheck disable=SC2154
# shellcheck source=/dev/null
source "${BATS_TEST_DIRNAME}/tests_common.sh"
# shellcheck source=/dev/null
source "${BATS_TEST_DIRNAME}/../../common.bash"
# shellcheck source=/dev/null
source "${BATS_TEST_DIRNAME}/../../hypervisor_helpers.sh"

load "${BATS_TEST_DIRNAME}/confidential_kbs.sh"

function setup_unencrypted_confidential_pod() {
	get_pod_config_dir

	# shellcheck disable=SC2154
	export SSH_KEY_FILE="${pod_config_dir}/confidential/unencrypted/ssh/unencrypted"

	if [[ -n "${GH_PR_NUMBER}" ]]; then
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
	# shellcheck disable=SC2154
	if is_se_hypervisor "${KATA_HYPERVISOR}"; then
		echo "cd /sys/firmware/uv; cat prot_virt_guest | grep 1"
	elif is_snp_hypervisor "${KATA_HYPERVISOR}"; then
		echo "dmesg | grep \"Memory Encryption Features active:.*SEV-SNP\""
	elif is_tdx_hypervisor "${KATA_HYPERVISOR}"; then
		echo "cpuid | grep TDX_GUEST"
	elif is_cca_hypervisor "${KATA_HYPERVISOR}"; then
		echo "echo 'Remote TEE verification is not implemented for qemu-cca' >&2; exit 1"
	else
		echo ""
	fi
}

# Common check for confidential GPU hardware tests.
function is_confidential_gpu_hardware() {
	if is_confidential_gpu_hypervisor "${KATA_HYPERVISOR}"; then
		return 0
	fi

	return 1
}

# create_loop_device creates a loop device backed by a file.
# $1: loop file path (default: /tmp/trusted-image-storage.img)
# $2: size in MiB, i.e. dd bs=1M count=... (default: 2500, ~2.4Gi)
function create_loop_device(){
	local loop_file="${1:-/tmp/trusted-image-storage.img}"
	local size_mb="${2:-2500}"
	local node
	node="$(get_one_kata_node)"
	cleanup_loop_device "${loop_file}"

	exec_host "${node}" "dd if=/dev/zero of=${loop_file} bs=1M count=${size_mb}"
	exec_host "${node}" "losetup -fP ${loop_file} >/dev/null 2>&1"
	local device
	device=$(exec_host "${node}" losetup -j "${loop_file}" | awk -F'[: ]' '{print $1}')

	echo "${device}"
}

function cleanup_loop_device(){
	local loop_file="${1:-/tmp/trusted-image-storage.img}"
	local node
	node="$(get_one_kata_node)"
	local existed_devices
	existed_devices=$(exec_host "${node}" losetup -j "${loop_file}" | awk -F'[: ]' '{print $1}')

	if [[ -n "${existed_devices}" ]]; then
		for d in ${existed_devices}; do
			exec_host "${node}" "losetup -d ${d} >/dev/null 2>&1"
		done
	fi

	exec_host "${node}" "rm -f ${loop_file} >/dev/null 2>&1 || true"
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
	CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
	export CC_KBS_ADDR

	kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
	kernel_params_value=""

	if [[ -n "${image_policy}" ]]; then
		kernel_params_value+=" agent.image_policy_file=${image_policy}"
		kernel_params_value+=" agent.enable_signature_verification=true"
	fi

	if [[ -n "${image_registry_auth}" ]]; then
		kernel_params_value+=" agent.image_registry_auth=${image_registry_auth}"
	fi

	if [[ -n "${guest_components_procs}" ]]; then
		kernel_params_value+=" agent.guest_components_procs=${guest_components_procs}"
	fi

	if [[ -n "${guest_components_rest_api}" ]]; then
		kernel_params_value+=" agent.guest_components_rest_api=${guest_components_rest_api}"
	fi

	kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"

	# Note: this is not local as we use it in the caller test
	kata_pod="$(new_pod_config "${image}" "kata-${KATA_HYPERVISOR}")"
	set_container_command "${kata_pod}" "0" "sleep" "30"

	# Set annotations
	set_metadata_annotation "${kata_pod}" \
		"io.containerd.cri.runtime-handler" \
		"kata-${KATA_HYPERVISOR}"
	set_metadata_annotation "${kata_pod}" \
		"${kernel_params_annotation}" \
		"${kernel_params_value}"

	add_allow_all_policy_to_yaml "${kata_pod}"

	if [[ -n "${node}" ]]; then
		set_node "${kata_pod}" "${node}"
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
	kata_pod="$(new_pod_config "${image}" "kata-${KATA_HYPERVISOR}")"
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

	if [[ -n "${node}" ]]; then
		set_node "${kata_pod}" "${node}"
	fi
}

# Sealed secrets (signed JWS ES256). Pre-created with guest-components secret CLI; see
# https://github.com/confidential-containers/guest-components/blob/main/confidential-data-hub/docs/SEALED_SECRET.md
# Tests provision the signing public key to KBS and use these pre-created sealed secret strings.
#
# To regenerate the signing key and sealed secrets:
# Install required dependencies, clone guest-components repository and change to guest-components/confidential-data-hub
# Create private and public JWK, for example:
# python3 -c "
# from jwcrypto import jwk
# k = jwk.JWK.generate(kty='EC', crv='P-256', alg='ES256', use='sig', kid='sealed-secret-test-key')
# with open('signing-key-private.jwk', 'w') as f:
#     f.write(k.export_private())
# with open('signing-key-public.jwk', 'w') as f:
#     f.write(k.export_public())
# print('Created signing-key-private.jwk and signing-key-public.jwk')
# "
#
# Build the secret CLI:
# cargo build -p confidential-data-hub --bin secret
#
# Create the sealed secret test secret:
# cargo run -p confidential-data-hub --bin secret -q -- seal \
#   --signing-kid "kbs:///default/signing-key/sealed-secret" \
#   --signing-jwk-path ./signing-key-private.jwk \
#   vault --resource-uri "kbs:///default/sealed-secret/test" --provider kbs
#
# Create the NIM test instruct secret:
# cargo run -p confidential-data-hub --bin secret -q -- seal \
#   --signing-kid "kbs:///default/signing-key/sealed-secret" \
#   --signing-jwk-path ./signing-key-private.jwk \
#   vault --resource-uri "kbs:///default/ngc-api-key/instruct" --provider kbs
#
# Create the NIM test embedqa secret:
# cargo run -p confidential-data-hub --bin secret -q -- seal \
#   --signing-kid "kbs:///default/signing-key/sealed-secret" \
#   --signing-jwk-path ./signing-key-private.jwk \
#   vault --resource-uri "kbs:///default/ngc-api-key/embedqa" --provider kbs
#
# Public JWK (no private key) used to verify the pre-created sealed secrets. Must match the key pair
# that was used to sign SEALED_SECRET_PRECREATED_*.
SEALED_SECRET_SIGNING_PUBLIC_JWK='{"alg":"ES256","crv":"P-256","kid":"sealed-secret-test-key","kty":"EC","use":"sig","x":"4jH376AuwTUCIx65AJ_56D7SZzWf7sGcEA7_Csq21UM","y":"rjdceysnSa5ZfzWOPGCURMUuHndxBAGUu4ISTIVN0yA"}'

# Pre-created sealed secret for k8s-sealed-secret.bats (points to kbs:///default/sealed-secret/test)
export SEALED_SECRET_PRECREATED_TEST="sealed.eyJiNjQiOnRydWUsImFsZyI6IkVTMjU2Iiwia2lkIjoia2JzOi8vL2RlZmF1bHQvc2lnbmluZy1rZXkvc2VhbGVkLXNlY3JldCJ9.eyJ2ZXJzaW9uIjoiMC4xLjAiLCJ0eXBlIjoidmF1bHQiLCJuYW1lIjoia2JzOi8vL2RlZmF1bHQvc2VhbGVkLXNlY3JldC90ZXN0IiwicHJvdmlkZXIiOiJrYnMiLCJwcm92aWRlcl9zZXR0aW5ncyI6e30sImFubm90YXRpb25zIjp7fX0.ZI2fTv5ramHqHQa9DKBFD5hlJ_Mjf6cEIcpsNGshpyhEiKklML0abfH600TD7LAFHf53oDIJmEcVsDtJ20UafQ"

# Pre-created sealed secrets for k8s-nvidia-nim.bats (point to kbs:///default/ngc-api-key/instruct and embedqa)
export SEALED_SECRET_PRECREATED_NIM_INSTRUCT="sealed.eyJiNjQiOnRydWUsImFsZyI6IkVTMjU2Iiwia2lkIjoia2JzOi8vL2RlZmF1bHQvc2lnbmluZy1rZXkvc2VhbGVkLXNlY3JldCJ9.eyJ2ZXJzaW9uIjoiMC4xLjAiLCJ0eXBlIjoidmF1bHQiLCJuYW1lIjoia2JzOi8vL2RlZmF1bHQvbmdjLWFwaS1rZXkvaW5zdHJ1Y3QiLCJwcm92aWRlciI6ImticyIsInByb3ZpZGVyX3NldHRpbmdzIjp7fSwiYW5ub3RhdGlvbnMiOnt9fQ.wpqvVFUaQymqgf54h70shZWDpk2NLW305wALz09YF0GKFBKBQiQB2sRwvn9Jk_rSju3YGLYxPO2Ub8qUbiMCuA"
export SEALED_SECRET_PRECREATED_NIM_EMBEDQA="sealed.eyJiNjQiOnRydWUsImFsZyI6IkVTMjU2Iiwia2lkIjoia2JzOi8vL2RlZmF1bHQvc2lnbmluZy1rZXkvc2VhbGVkLXNlY3JldCJ9.eyJ2ZXJzaW9uIjoiMC4xLjAiLCJ0eXBlIjoidmF1bHQiLCJuYW1lIjoia2JzOi8vL2RlZmF1bHQvbmdjLWFwaS1rZXkvZW1iZWRxYSIsInByb3ZpZGVyIjoia2JzIiwicHJvdmlkZXJfc2V0dGluZ3MiOnt9LCJhbm5vdGF0aW9ucyI6e319.4C1uqtVXi_qZT8vh_yZ4KpsRdgr2s4hU6ElKj18Hq1DJi_Iji61yuKsS6S1jWdb7drdoKKACvMD6RmCd85SJOQ"

# Set up KBS with image security policy requiring cosign signatures using the NVIDIA
# container signing public key. When initdata includes image_security_policy_uri
# pointing to this policy, the guest will verify image signatures when it pulls the image.
# Note: keyPath-only sigstoreSigned does not require Rekor per spec; if the guest fails
# with "signature not found in transparency log", the in-guest verifier may need to
# support key-only verification (host-side: cosign verify --key <pubkey> --insecure-ignore-tlog <image>).
function setup_kbs_nim_image_policy() {
	local policy_json public_key
	# Cosign public key for nvcr.io images. Source: NVIDIA NGC Catalog API.
	# See https://docs.nvidia.com/ngc/latest/ngc-catalog-user-guide.html#finding-nvidia-s-public-key
	public_key=$(curl -sSL "https://api.ngc.nvidia.com/v2/catalog/containers/public-key")
	policy_json=$(cat << EOF
{
    "default": [{"type": "reject"}],
    "transports": {
        "docker": {
            "nvcr.io/nim/meta": [
                {
                    "type": "sigstoreSigned",
                    "keyPath": "kbs:///default/cosign-public-key/nim",
                    "signedIdentity": {"type": "matchRepository"}
                }
            ],
            "nvcr.io/nim/nvidia": [
                {
                    "type": "sigstoreSigned",
                    "keyPath": "kbs:///default/cosign-public-key/nim",
                    "signedIdentity": {"type": "matchRepository"}
                }
            ]
        }
    }
}
EOF
	)
	kbs_set_resource "default" "security-policy" "nim" "${policy_json}"
	kbs_set_resource "default" "cosign-public-key" "nim" "${public_key}"
}

# Provision the signing public key to KBS so CDH can verify the pre-created sealed secrets.
function setup_sealed_secret_signing_public_key() {
	kbs_set_resource "default" "signing-key" "sealed-secret" "${SEALED_SECRET_SIGNING_PUBLIC_JWK}"
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

[log]
level = "debug"
'''

"cdh.toml" = '''
[kbc]
name = "cc_kbc"
url = "${CC_KBS_ADDRESS}"

[log]
level = "debug"

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
	teardown_common "${node}" "${node_start_time}"

	# Also try and print the kbs logs on failure
	if [[ -n "${node_start_time}" && -z "${BATS_TEST_COMPLETED}" ]]; then
		kbs_k8s_print_logs "${node_start_time}"
	fi
}
