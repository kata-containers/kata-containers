#!/usr/bin/env bats
# Copyright (c) 2024 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KBS="${KBS:-false}"

setup() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    if [ "${KBS}" = "false" ]; then
        skip "Test skipped as KBS not setup"
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    setup_common || die "setup_common failed"
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

@test "Test that creating a container from an encrypted image, with no decryption key fails" {

    # TODO - there is now delete KBS resource to ensure there is no key, so we need to keep
    # this test running first to ensure that the KBS doesn't have the resource. An alternative
    # is to run kbs_set_deny_all_resources, but we don't have a way to reset to the default
    # policy, so for TEE tests we'd stay remaining with reject all, which could cause other
    # subsequent tests to fail

    create_coco_pod_yaml "${ENCRYPTED_IMAGE}" "" "" "confidential-data-hub" "" "$node"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    assert_pod_fail "${kata_pod}"
    assert_logs_contain "${node}" kata "${node_start_time}" 'failed to get decrypt key'
    assert_logs_contain "${node}" kata "${node_start_time}" 'no suitable key found for decrypting layer key'
}


@test "Test that creating a container from an encrypted image, with correct decryption key works" {

    setup_kbs_decryption_key "${DECRYPTION_KEY}" "${DECRYPTION_KEY_ID}"

    create_coco_pod_yaml "${ENCRYPTED_IMAGE}" "" "" "confidential-data-hub" "" "$node"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    k8s_create_pod "${kata_pod}"
    echo "Kata pod test-e2e from encrypted image is running"
}

@test "Test that creating a container from an encrypted image, with incorrect decryption key fails" {

    setup_kbs_decryption_key "anVua19rZXk=" "${DECRYPTION_KEY_ID}"

    create_coco_pod_yaml "${ENCRYPTED_IMAGE}" "" "" "confidential-data-hub" "" "$node"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    assert_pod_fail "${kata_pod}"
    assert_logs_contain "${node}" kata "${node_start_time}" 'failed to get decrypt key'
    assert_logs_contain "${node}" kata "${node_start_time}" 'no suitable key found for decrypting layer key'
}

teardown() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    if [ "${KBS}" = "false" ]; then
        skip "Test skipped as KBS not setup"
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    teardown_common "${node}" "${node_start_time:-}"
}
