#!/usr/bin/env bats
# Copyright (c) 2024 IBM Corporation
# Copyright (c) 2024 Alibaba Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/confidential_kbs.sh"

export KBS="${KBS:-false}"

setup() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    setup_common
    UNSIGNED_UNPROTECTED_REGISTRY_IMAGE="quay.io/prometheus/busybox:latest"
    UNSIGNED_PROTECTED_REGISTRY_IMAGE="ghcr.io/confidential-containers/test-container-image-rs:unsigned"
    COSIGN_SIGNED_PROTECTED_REGISTRY_IMAGE="ghcr.io/confidential-containers/test-container-image-rs:cosign-signed"
    COSIGNED_SIGNED_PROTECTED_REGISTRY_WRONG_KEY_IMAGE="ghcr.io/confidential-containers/test-container-image-rs:cosign-signed-key2"
}

function setup_kbs_image_policy() {
    if [ "${KBS}" = "false" ]; then
        skip "Test skipped as KBS not setup"
    fi

    default_policy="${1:-insecureAcceptAnything}"
    policy_json=$(cat << EOF
{                 
    "default": [
        {                                      
        "type": "${default_policy}"
        }
    ],                 
    "transports": {
        "docker": {                                                           
            "ghcr.io/confidential-containers/test-container-image-rs": [
                {                                        
                    "type": "sigstoreSigned",
                    "keyPath": "kbs:///default/cosign-public-key/test"
                }     
            ],                             
            "quay.io/prometheus": [
                {                                               
                    "type": "insecureAcceptAnything"
                }    
            ]
        }
    }
}
EOF
    )

    # This public key is corresponding to a private key that was generated to test signed images in image-rs CI.
    # TODO: Update the CI to generate a signed image together with verification. See issue #9360
    public_key=$(curl -sSL "https://raw.githubusercontent.com/confidential-containers/guest-components/075b9a9ee77227d9d92b6f3649ef69de5e72d204/image-rs/test_data/signature/cosign/cosign1.pub")

    if ! is_confidential_hardware; then
        kbs_set_allow_all_resources
    fi

    kbs_set_resource "default" "security-policy" "test" "${policy_json}"
    kbs_set_resource "default" "cosign-public-key" "test" "${public_key}"
}

function create_pod_yaml_with_signed_image() {
    image=$1
    image_security=${2:-true}

    local CC_KBS_ADDR
    export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
    kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
    kernel_params_value="agent.guest_components_rest_api=resource"

    if [[ $image_security == true ]]; then
        kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
        kernel_params_value+=" agent.enable_signature_verification=true"
        kernel_params_value+=" agent.image_policy_file=kbs:///default/security-policy/test"
    fi

    # Note: this is not local as we use it in the caller test
    kata_pod_with_signed_image="$(new_pod_config "$image" "kata-${KATA_HYPERVISOR}")"
    set_container_command "${kata_pod_with_signed_image}" "0" "sleep" "30"

    # Set annotation to pull image in guest
    set_metadata_annotation "${kata_pod_with_signed_image}" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"
    set_metadata_annotation "${kata_pod_with_signed_image}" \
        "${kernel_params_annotation}" \
        "${kernel_params_value}"

    add_allow_all_policy_to_yaml "${kata_pod_with_signed_image}"
}

@test "Create a pod from an unsigned image, on an insecureAcceptAnything registry works" {
    # We want to set the default policy to be reject to rule out false positives
    setup_kbs_image_policy "reject"

    create_pod_yaml_with_signed_image "${UNSIGNED_UNPROTECTED_REGISTRY_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_signed_image}: $(cat ${kata_pod_with_signed_image})"

    k8s_create_pod "${kata_pod_with_signed_image}"
    echo "Kata pod test-e2e from image security policy is running"
}

@test "Create a pod from an unsigned image, on a 'restricted registry' is rejected" {
    # We want to leave the default policy to be insecureAcceptAnything to rule out false negatives
    setup_kbs_image_policy

    create_pod_yaml_with_signed_image "${UNSIGNED_PROTECTED_REGISTRY_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_signed_image}: $(cat ${kata_pod_with_signed_image})"

    assert_pod_fail "${kata_pod_with_signed_image}"
    assert_logs_contain "${node}" kata "${node_start_time}" "Security validate failed: Validate image failed: Cannot pull manifest"
}

@test "Create a pod from a signed image, on a 'restricted registry' is successful" {
    # We want to set the default policy to be reject to rule out false positives
    setup_kbs_image_policy "reject"

    create_pod_yaml_with_signed_image "${COSIGN_SIGNED_PROTECTED_REGISTRY_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_signed_image}: $(cat ${kata_pod_with_signed_image})"

    k8s_create_pod "${kata_pod_with_signed_image}"
    echo "Kata pod test-e2e from image security policy is running"
}

@test "Create a pod from a signed image, on a 'restricted registry', but with the wrong key is rejected" {
    # We want to leave the default policy to be insecureAcceptAnything to rule out false negatives
    setup_kbs_image_policy

    create_pod_yaml_with_signed_image "${COSIGNED_SIGNED_PROTECTED_REGISTRY_WRONG_KEY_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_signed_image}: $(cat ${kata_pod_with_signed_image})"

    assert_pod_fail "${kata_pod_with_signed_image}"
    assert_logs_contain "${node}" kata "${node_start_time}" "Security validate failed: Validate image failed: \[PublicKeyVerifier"
}

@test "Create a pod from an unsigned image, on a 'restricted registry' works if enable_signature_verfication is false" {
    # We want to set the default policy to be reject to rule out false positives
    setup_kbs_image_policy "reject"

    create_pod_yaml_with_signed_image "${UNSIGNED_PROTECTED_REGISTRY_IMAGE}" "false"

    # For debug sake
    echo "Pod ${kata_pod_with_signed_image}: $(cat ${kata_pod_with_signed_image})"

    k8s_create_pod "${kata_pod_with_signed_image}"
    echo "Kata pod test-e2e from image security policy is running"
}

teardown() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    kubectl describe pods
    k8s_delete_all_pods_if_any_exists || true

    if [[ -n "${node_start_time:-}" && -z "$BATS_TEST_COMPLETED" ]]; then
		echo "DEBUG: system logs of node '$node' since test start time ($node_start_time)"
		print_node_journal "$node" "kata" --since "$node_start_time" || true
	fi
}
