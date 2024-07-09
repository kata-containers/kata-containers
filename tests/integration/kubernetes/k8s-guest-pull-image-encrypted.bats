#!/usr/bin/env bats
# Copyright (c) 2024 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/confidential_kbs.sh"

export KBS="${KBS:-false}"

setup() {
    if is_confidential_hardware; then
        skip "Due to issues related to pull-image integration skip tests for ${KATA_HYPERVISOR}."
    fi

    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    if [ "${KBS}" = "false" ]; then
        skip "Test skipped as KBS not setup"
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    setup_common
    ENCRYPTED_IMAGE="${ENCRYPTED_IMAGE:-ghcr.io/confidential-containers/test-container:multi-arch-encrypted}"
    DECRYPTION_KEY="${DECRYPTION_KEY:-HUlOu8NWz8si11OZUzUJMnjiq/iZyHBJZMSD3BaqgMc=}"
    DECRYPTION_KEY_ID="${DECRYPTION_KEY_ID:-ssh-demo}"
}

function setup_kbs_decryption_key() {
    decryption_key=$1
    decryption_key_id=$2

    if ! is_confidential_hardware; then
        kbs_set_allow_all_resources
    fi

    # Note: the ssh-demo
    kbs_set_resource_base64 "default" "key" "${decryption_key_id}" "${decryption_key}"
}

function create_pod_yaml_with_encrypted_image() {
    image=$1

    # Note: this is not local as we use it in the caller test
    kata_pod_with_encrypted_image="$(new_pod_config "$image" "kata-${KATA_HYPERVISOR}")"
    set_node "${kata_pod_with_encrypted_image}" "$node"
    set_container_command "${kata_pod_with_encrypted_image}" "0" "sleep" "30"

    local CC_KBS_ADDR
    export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
    kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
    kernel_params_value+=" agent.guest_components_procs=confidential-data-hub"
    kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"

    set_metadata_annotation "${kata_pod_with_encrypted_image}" \
        "${kernel_params_annotation}" \
        "${kernel_params_value}"

    # Set annotation to pull image in guest
    set_metadata_annotation "${kata_pod_with_encrypted_image}" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    add_allow_all_policy_to_yaml "${kata_pod_with_encrypted_image}"
}

@test "Test that creating a container from an encrypted image, with no decryption key fails" {

    # TODO - there is now delete KBS resource to ensure there is no key, so we need to keep
    # this test running first to ensure that the KBS doesn't have the resource. An alternative
    # is to run kbs_set_deny_all_resources, but we don't have a way to reset to the default
    # policy, so for TEE tests we'd stay remaining with reject all, which could cause other
    # subsequent tests to fail

    create_pod_yaml_with_encrypted_image "${ENCRYPTED_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_encrypted_image}: $(cat ${kata_pod_with_encrypted_image})"

    assert_pod_fail "${kata_pod_with_encrypted_image}"
    assert_logs_contain "${node}" kata "${node_start_time}" 'failed to get decrypt key missing private key needed for decryption'
}


@test "Test that creating a container from an encrypted image, with correct decryption key works" {

    setup_kbs_decryption_key "${DECRYPTION_KEY}" "${DECRYPTION_KEY_ID}"

    create_pod_yaml_with_encrypted_image "${ENCRYPTED_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_encrypted_image}: $(cat ${kata_pod_with_encrypted_image})"

    k8s_create_pod "${kata_pod_with_encrypted_image}"
    echo "Kata pod test-e2e from encrypted image is running"
}

@test "Test that creating a container from an encrypted image, with incorrect decryption key fails" {

    setup_kbs_decryption_key "anVua19rZXk=" "${DECRYPTION_KEY_ID}"

    create_pod_yaml_with_encrypted_image "${ENCRYPTED_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_encrypted_image}: $(cat ${kata_pod_with_encrypted_image})"

    assert_pod_fail "${kata_pod_with_encrypted_image}"
    assert_logs_contain "${node}" kata "${node_start_time}" 'failed to get decrypt key missing private key needed for decryption'
}

teardown() {
    if is_confidential_hardware; then
        skip "Due to issues related to pull-image integration skip tests for ${KATA_HYPERVISOR}."
    fi

    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    if [ "${KBS}" = "false" ]; then
        skip "Test skipped as KBS not setup"
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    kubectl describe pods
    k8s_delete_all_pods_if_any_exists || true

    if [[ -n "${node_start_time}:-}" && -z "$BATS_TEST_COMPLETED" ]]; then
        echo "DEBUG: system logs of node '$node' since test start time ($node_start_time)"
        print_node_journal "$node" "kata" --since "$node_start_time" || true
    fi
}
