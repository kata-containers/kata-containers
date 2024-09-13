#!/usr/bin/env bats
# Copyright (c) 2023 Intel Corporation
# Copyright (c) 2023 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

setup() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    setup_common
    get_pod_config_dir
    unencrypted_image="quay.io/prometheus/prometheus:latest"
    image_pulled_time_less_than_default_time="ghcr.io/confidential-containers/test-container:rust-1.79.0" # unpacked size: 1.41GB
    large_image="quay.io/confidential-containers/test-images:largeimage" # unpacked size: 2.15GB
    pod_config_template="${pod_config_dir}/pod-guest-pull-in-trusted-storage.yaml.in"
    storage_config_template="${pod_config_dir}/confidential/trusted-storage.yaml.in"
}

@test "Test we can pull an unencrypted image outside the guest with runc and then inside the guest successfully" {
    # 1. Create one runc pod with the $unencrypted_image image
    # We want to have one runc pod, so we pass a fake runtimeclass "runc" and then delete the runtimeClassName,
    # because the runtimeclass is not optional in new_pod_config function.
    runc_pod_config="$(new_pod_config "$unencrypted_image" "runc")"
    sed -i '/runtimeClassName:/d' $runc_pod_config
    set_node "$runc_pod_config" "$node"
    set_container_command "$runc_pod_config" "0" "sleep" "30"

    # For debug sake
    echo "Pod $runc_pod_config file:"
    cat $runc_pod_config

    add_allow_all_policy_to_yaml "$runc_pod_config"
    k8s_create_pod "$runc_pod_config"

    echo "Runc pod test-e2e is running"
    kubectl delete -f "$runc_pod_config"

    # 2. Create one kata pod with the $unencrypted_image image and nydus annotation
    kata_pod_with_nydus_config="$(new_pod_config "$unencrypted_image" "kata-${KATA_HYPERVISOR}")"
    set_node "$kata_pod_with_nydus_config" "$node"
    set_container_command "$kata_pod_with_nydus_config" "0" "sleep" "30"

    # Set annotation to pull image in guest
    set_metadata_annotation "$kata_pod_with_nydus_config" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    # For debug sake
    echo "Pod $kata_pod_with_nydus_config file:"
    cat $kata_pod_with_nydus_config

    add_allow_all_policy_to_yaml "$kata_pod_with_nydus_config"
    k8s_create_pod "$kata_pod_with_nydus_config"
}

teardown() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    kubectl describe pods
    k8s_delete_all_pods_if_any_exists || true
    kubectl delete --ignore-not-found pvc trusted-pvc
    kubectl delete --ignore-not-found pv trusted-block-pv
    kubectl delete --ignore-not-found storageclass local-storage
    cleanup_loop_device || true
}
