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

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    setup_common || die "setup_common failed"
    AUTHENTICATED_IMAGE="${AUTHENTICATED_IMAGE:-quay.io/kata-containers/confidential-containers-auth:test}"
    AUTHENTICATED_IMAGE_USER=${AUTHENTICATED_IMAGE_USER:-}
    AUTHENTICATED_IMAGE_PASSWORD=${AUTHENTICATED_IMAGE_PASSWORD:-}

    if [[ -z ${AUTHENTICATED_IMAGE_USER} || -z ${AUTHENTICATED_IMAGE_PASSWORD} ]]; then
        if [[ -n ${GITHUB_ACTION:-} ]]; then
            die "User and/or password not supplied to authenticated registry test"
        else
            skip "running test locally due to missing user/password"
        fi
    fi

    # Set up Kubernetes secret for the nydus-snapshotter metadata pull
    kubectl delete secret cococred --ignore-not-found
    kubectl create secret docker-registry cococred --docker-server="https://"$(echo "$AUTHENTICATED_IMAGE" | cut -d':' -f1) \
    --docker-username="${AUTHENTICATED_IMAGE_USER}" --docker-password="${AUTHENTICATED_IMAGE_PASSWORD}"
}

function setup_kbs_credentials() {
    image=$1
    user=$2
    password=$3

    if [ "${KBS}" = "false" ]; then
        skip "Test skipped as KBS not setup"
    fi

    registry_credential_encoded=$(echo "${user}:${password}" | base64 -w 0)
    registry=$(echo "$image" | cut -d':' -f1)

    auth_json=$(echo "{
    \"auths\": {
        \"${registry}\": {
            \"auth\": \"${registry_credential_encoded}\"
        }
    }
}")

    if ! is_confidential_hardware; then
        kbs_set_allow_all_resources
    fi

    kbs_set_resource "default" "credentials" "test" "${auth_json}"
}

@test "Test that creating a container from an authenticated image, with correct credentials works" {

    setup_kbs_credentials "${AUTHENTICATED_IMAGE}" ${AUTHENTICATED_IMAGE_USER} ${AUTHENTICATED_IMAGE_PASSWORD}

    create_coco_pod_yaml "${AUTHENTICATED_IMAGE}" "" "kbs:///default/credentials/test" "" "resource" "$node"
    yq -i ".spec.imagePullSecrets[0].name = \"cococred\"" "${kata_pod}"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    k8s_create_pod "${kata_pod}"
    echo "Kata pod test-e2e from authenticated image is running"
}

@test "Test that creating a container from an authenticated image, with incorrect credentials fails" {

    setup_kbs_credentials "${AUTHENTICATED_IMAGE}" ${AUTHENTICATED_IMAGE_USER} "junk"

    create_coco_pod_yaml "${AUTHENTICATED_IMAGE}" "" "kbs:///default/credentials/test" "" "resource" "$node"
    yq -i ".spec.imagePullSecrets[0].name = \"cococred\"" "${kata_pod}"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    assert_pod_fail "${kata_pod}"
    assert_logs_contain "${node}" kata "${node_start_time}" "failed to pull manifest Not authorized"
}

@test "Test that creating a container from an authenticated image, with no credentials fails" {

    # Create pod config, but don't add agent.image_registry_auth annotation
    create_coco_pod_yaml "${AUTHENTICATED_IMAGE}" "" "" "" "resource" "$node"
    yq -i ".spec.imagePullSecrets[0].name = \"cococred\"" "${kata_pod}"

    # For debug sake
    echo "Pod ${kata_pod}: $(cat ${kata_pod})"

    assert_pod_fail "${kata_pod}"
    assert_logs_contain "${node}" kata "${node_start_time}" "failed to pull manifest Not authorized"
}

teardown() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    teardown_common "${node}" "${node_start_time:-}"
    kubectl delete secret cococred --ignore-not-found
}
