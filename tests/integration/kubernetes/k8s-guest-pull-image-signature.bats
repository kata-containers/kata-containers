#!/usr/bin/env bats
# Copyright (c) 2024 IBM Corporation
# Copyright (c) 2024 Alibaba Corporation
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

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    tag_suffix=""
    if [ "$(uname -m)" != "x86_64" ]; then
        tag_suffix="-$(uname -m)"
    fi

    setup_common || die "setup_common failed"
    UNSIGNED_UNPROTECTED_REGISTRY_IMAGE="quay.io/prometheus/busybox:latest"
    UNSIGNED_PROTECTED_REGISTRY_IMAGE="ghcr.io/confidential-containers/test-container-image-rs:unsigned${tag_suffix}"
    COSIGN_SIGNED_PROTECTED_REGISTRY_IMAGE="ghcr.io/confidential-containers/test-container-image-rs:cosign-signed${tag_suffix}"
    COSIGNED_SIGNED_PROTECTED_REGISTRY_WRONG_KEY_IMAGE="ghcr.io/confidential-containers/test-container-image-rs:cosign-signed-key2${tag_suffix}"
    SECURITY_POLICY_KBS_URI="kbs:///default/security-policy/test"
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

@test "Create a pod from an unsigned image, on an insecureAcceptAnything registry works" {
    # We want to set the default policy to be reject to rule out false positives
    setup_kbs_image_policy "reject"

    create_coco_pod_yaml "${UNSIGNED_UNPROTECTED_REGISTRY_IMAGE}" "${SECURITY_POLICY_KBS_URI}" "" "" "resource" "$node"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    k8s_create_pod "${kata_pod}"
    echo "Kata pod test-e2e from image security policy is running"
}

@test "Create a pod from an unsigned image, on a 'restricted registry' is rejected" {
    # We want to leave the default policy to be insecureAcceptAnything to rule out false negatives
    setup_kbs_image_policy

    create_coco_pod_yaml "${UNSIGNED_PROTECTED_REGISTRY_IMAGE}" "${SECURITY_POLICY_KBS_URI}" "" "" "resource" "$node"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    assert_pod_fail "${kata_pod}"
    assert_logs_contain "${node}" kata "${node_start_time}" "image security validation failed"
}

@test "Create a pod from a signed image, on a 'restricted registry' is successful" {
    # We want to set the default policy to be reject to rule out false positives
    setup_kbs_image_policy "reject"

    create_coco_pod_yaml "${COSIGN_SIGNED_PROTECTED_REGISTRY_IMAGE}" "${SECURITY_POLICY_KBS_URI}" "" "" "resource" "$node"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    k8s_create_pod "${kata_pod}"
    echo "Kata pod test-e2e from image security policy is running"
}

@test "Create a pod from a signed image, on a 'restricted registry', but with the wrong key is rejected" {
    # We want to leave the default policy to be insecureAcceptAnything to rule out false negatives
    setup_kbs_image_policy

    create_coco_pod_yaml "${COSIGNED_SIGNED_PROTECTED_REGISTRY_WRONG_KEY_IMAGE}" "${SECURITY_POLICY_KBS_URI}" "" "" "resource" "$node"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    assert_pod_fail "${kata_pod}"
    assert_logs_contain "${node}" kata "${node_start_time}" "image security validation failed"
}

@test "Create a pod from an unsigned image, on a 'restricted registry' works if policy files isn't set" {
    # We want to set the default policy to be reject to rule out false positives
    setup_kbs_image_policy "reject"

    create_coco_pod_yaml "${UNSIGNED_PROTECTED_REGISTRY_IMAGE}" "" "" "" "resource" "$node"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    k8s_create_pod "${kata_pod}"
    echo "Kata pod test-e2e from image security policy is running"
}

teardown() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    teardown_common "${node}" "${node_start_time:-}"
}
