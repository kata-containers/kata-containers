#!/usr/bin/env bats
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

setup() {
    [[ "${PULL_TYPE}" =~ "host-share-image-block" ]] || skip "Test only working for sharing image with verity on the host"
    confidential_setup || skip "Test not supported for ${KATA_HYPERVISOR}."
    setup_common
    image_unsigned_unprotected_for_sharing="quay.io/chengyu_zhu/redis:alpine3.19"
}

@test "Test can pull an image as a raw block disk image to guest with dm-verity" {
    [[ " ${SUPPORTED_NON_TEE_HYPERVISORS} " =~ " ${KATA_HYPERVISOR} " ]] && skip "Test not supported for ${KATA_HYPERVISOR}."
    kata_pod_config="$(new_pod_config "$image_unsigned_unprotected_for_sharing" "kata-${KATA_HYPERVISOR}")"
    set_node "$kata_pod_config" "$node"

    # Set annotation to pull image
    set_metadata_annotation "$kata_pod_config" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    # For debug sake
    echo "Pod $kata_pod_config file:"
    cat $kata_pod_config

    k8s_create_pod "$kata_pod_config"
    echo "Kata pod test-e2e with nydus annotation is running"

    echo "Checking the image was shared to the guest with block devices"
    block_volumes_count=$(kubectl exec "test-e2e" -c test-container -- mount | grep "virtual-volumes" | wc -l)
    [ ${block_volumes_count} -ge 1 ]
}

@test "Test can create two pods by sharing the same image with dm-verity enabled" {
    [[ " ${SUPPORTED_NON_TEE_HYPERVISORS} " =~ " ${KATA_HYPERVISOR} " ]] && skip "Test not supported for ${KATA_HYPERVISOR}."
    kata_pod_config_1="$(new_pod_config "$image_unsigned_unprotected_for_sharing" "kata-${KATA_HYPERVISOR}")"
    set_node "$kata_pod_config_1" "$node"

    # Set annotation to pull image
    set_metadata_annotation "$kata_pod_config_1" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    # For debug sake
    echo "Pod $kata_pod_config_1 file:"
    cat $kata_pod_config_1

    kubectl create -f  "$kata_pod_config_1"
    kubectl wait --for=condition=Ready --timeout=$timeout pod "test-e2e"
    echo "Kata pod test-e2e is running"

    # Create another pod with the same container image
    kata_pod_config_2="$(new_pod_config "$image_unsigned_unprotected_for_sharing" "kata-${KATA_HYPERVISOR}")"
    set_node "$kata_pod_config_2" "$node"

    # Change the pod name in the yaml file
    sed -i "s/test-e2e/test-e2e-2/g" $kata_pod_config_2

    # Set annotation to pull image
    set_metadata_annotation "$kata_pod_config_1" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    # For debug sake
    echo "Pod $kata_pod_config_2 file:"
    cat $kata_pod_config_2

    kubectl create -f  "$kata_pod_config_2"
    kubectl wait --for=condition=Ready --timeout=$timeout pod "test-e2e-2"
    echo "Kata pod test-e2e-2 is running"

    echo "Checking two pods sharing the same image with block devices"
    lowdir_value_for_pod_1=$(kubectl exec "test-e2e" -c test-container -- mount | grep "virtual-volumes" | grep -oP 'lowerdir=\K[^,]*')
    lowdir_value_for_pod_2=$(kubectl exec "test-e2e-2" -c test-container -- mount | grep "virtual-volumes" | grep -oP 'lowerdir=\K[^,]*')
    [ "$lowdir_value_for_pod_1" == "$lowdir_value_for_pod_2" ]
}

teardown() {
    [[ "${PULL_TYPE}" =~ "host-share-image-block" ]] || skip "Test only working for sharing image with verity on the host"
    check_hypervisor_for_confidential_tests ${KATA_HYPERVISOR} || skip "Test not supported for ${KATA_HYPERVISOR}."
    k8s_delete_all_pods_if_any_exists || true
}
