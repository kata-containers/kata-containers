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
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    setup_common
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

function create_pod_yaml_with_private_image() {
    image=$1
    auth_path_set=${2:-true}

    # Note: this is not local as we use it in the caller test
    kata_pod_with_private_image="$(new_pod_config "$image" "kata-${KATA_HYPERVISOR}")"
    set_node "${kata_pod_with_private_image}" "$node"
    set_container_command "${kata_pod_with_private_image}" "0" "sleep" "30"

    local CC_KBS_ADDR
    export CC_KBS_ADDR=$(kbs_k8s_svc_http_addr)
    kernel_params_annotation="io.katacontainers.config.hypervisor.kernel_params"
    kernel_params_value="agent.guest_components_rest_api=resource"

    if [[ $auth_path_set == true ]]; then
        kernel_params_value+=" agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"
        kernel_params_value+=" agent.image_registry_auth=kbs:///default/credentials/test"
    fi
    set_metadata_annotation "${kata_pod_with_private_image}" \
        "${kernel_params_annotation}" \
        "${kernel_params_value}"

    # Set annotation to pull image in guest
    set_metadata_annotation "${kata_pod_with_private_image}" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    add_allow_all_policy_to_yaml "${kata_pod_with_private_image}"

    yq -i ".spec.imagePullSecrets[0].name = \"cococred\"" "${kata_pod_with_private_image}"
}

@test "Test that creating a container from an authenticated image, with correct credentials works" {

    setup_kbs_credentials "${AUTHENTICATED_IMAGE}" ${AUTHENTICATED_IMAGE_USER} ${AUTHENTICATED_IMAGE_PASSWORD}

    create_pod_yaml_with_private_image "${AUTHENTICATED_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_private_image}: $(cat ${kata_pod_with_private_image})"

    k8s_create_pod "${kata_pod_with_private_image}"
    echo "Kata pod test-e2e from authenticated image is running"
}

@test "Test that creating a container from an authenticated image, with incorrect credentials fails" {

    setup_kbs_credentials "${AUTHENTICATED_IMAGE}" ${AUTHENTICATED_IMAGE_USER} "junk"
    create_pod_yaml_with_private_image "${AUTHENTICATED_IMAGE}"

    # For debug sake
    echo "Pod ${kata_pod_with_private_image}: $(cat ${kata_pod_with_private_image})"

    assert_pod_fail "${kata_pod_with_private_image}"
    assert_logs_contain "${node}" kata "${node_start_time}" "failed to pull manifest Not authorized"
}

@test "Test that creating a container from an authenticated image, with no credentials fails" {

    # Create pod config, but don't add agent.image_registry_auth annotation
    create_pod_yaml_with_private_image "${AUTHENTICATED_IMAGE}" false

    # For debug sake
    echo "Pod ${kata_pod_with_private_image}: $(cat ${kata_pod_with_private_image})"

    assert_pod_fail "${kata_pod_with_private_image}"
    assert_logs_contain "${node}" kata "${node_start_time}" "failed to pull manifest Not authorized"
}

teardown() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    kubectl delete secret cococred --ignore-not-found

    kubectl describe pods
    k8s_delete_all_pods_if_any_exists || true

    if [[ -n "${node_start_time:-}" && -z "$BATS_TEST_COMPLETED" ]]; then
		echo "DEBUG: system logs of node '$node' since test start time ($node_start_time)"
		print_node_journal "$node" "kata" --since "$node_start_time" || true
	fi
}
