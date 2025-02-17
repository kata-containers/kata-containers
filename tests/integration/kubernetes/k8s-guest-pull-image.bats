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

    setup_common || die "setup_common failed"
    get_pod_config_dir
    unencrypted_image="quay.io/prometheus/busybox:latest"
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

@test "Test we cannot pull an image that exceeds the memory limit inside the guest" {
    # The image pulled in the guest will be downloaded and unpacked in the `/run/kata-containers/image` directory.
    # However, by default, systemd allocates 50% of the available physical RAM to the `/run` directory using a `tmpfs` filesystem.
    # It means that if we run a kata container with the default configuration (where the default memory assigned for a VM is 2048 MiB),
    # `/run` would be allocated around 1024 MiB. Consequently, we can only pull images up to 1024 MiB in the guest.
    # However, the unpacked size of image "ghcr.io/confidential-containers/test-container:rust-1.79.0" is 1.41GB.
    # It will fail to run the pod with pulling the image in the memory in the guest by default.

    pod_config="$(new_pod_config "$image_pulled_time_less_than_default_time" "kata-${KATA_HYPERVISOR}")"
    set_node "$pod_config" "$node"
    set_container_command "$pod_config" "0" "sleep" "30"

    # Set annotation to pull image in guest
    set_metadata_annotation "${pod_config}" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    # For debug sake
    echo "Pod $pod_config file:"
    cat $pod_config

    # The pod should be failed because the unpacked image size is larger than the memory size in the guest.
    assert_pod_fail "$pod_config"
    assert_logs_contain "$node" kata "$node_start_time" \
		'No space left on device'
}

@test "Test we can pull an image inside the guest using trusted storage" {
	[ "$(uname -m)" == "s390x" ] && skip "See: https://github.com/kata-containers/kata-containers/issues/10838"
    # The image pulled in the guest will be downloaded and unpacked in the `/run/kata-containers/image` directory.
    # The tests will use `cryptsetup` to encrypt a block device and mount it at `/run/kata-containers/image`.

    storage_config=$(mktemp "${BATS_FILE_TMPDIR}/$(basename "${storage_config_template}").XXX")
    local_device=$(create_loop_device)
    LOCAL_DEVICE="$local_device" NODE_NAME="$node" envsubst < "$storage_config_template" > "$storage_config"

    # For debug sake
    echo "Trusted storage $storage_config file:"
    cat $storage_config

    # Create persistent volume and persistent volume claim
    retry_kubectl_apply $storage_config

    pod_config=$(mktemp "${BATS_FILE_TMPDIR}/$(basename "${pod_config_template}").XXX")
    IMAGE="$image_pulled_time_less_than_default_time" NODE_NAME="$node" envsubst < "$pod_config_template" > "$pod_config"


    # Set CreateContainerRequest timeout for qemu-coco-dev
    if [ "${KATA_HYPERVISOR}" == "qemu-coco-dev" ]; then
        create_container_timeout=300
        set_metadata_annotation "$pod_config" \
            "io.katacontainers.config.runtime.create_container_timeout" \
            "${create_container_timeout}"
    fi

    # Enable dm-integrity in guest
    set_metadata_annotation "${pod_config}" \
        "io.katacontainers.config.hypervisor.kernel_params" \
        "agent.secure_storage_integrity=true"

    # Set annotation to pull image in guest
    set_metadata_annotation "${pod_config}" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    # For debug sake
    echo "Pod $pod_config file:"
    cat $pod_config

    add_allow_all_policy_to_yaml "$pod_config"
    local wait_time=120
    [ "${KATA_HYPERVISOR}" == "qemu-coco-dev" ] && wait_time=300
    k8s_create_pod "$pod_config" "$wait_time"
}

@test "Test we cannot pull a large image that pull time exceeds createcontainer timeout inside the guest" {
	[ "$(uname -m)" == "s390x" ] && skip "See: https://github.com/kata-containers/kata-containers/issues/10838"

    storage_config=$(mktemp "${BATS_FILE_TMPDIR}/$(basename "${storage_config_template}").XXX")
    local_device=$(create_loop_device)
    LOCAL_DEVICE="$local_device" NODE_NAME="$node" envsubst < "$storage_config_template" > "$storage_config"

    # For debug sake
    echo "Trusted storage $storage_config file:"
    cat $storage_config

    # Create persistent volume and persistent volume claim
    retry_kubectl_apply $storage_config

    pod_config=$(mktemp "${BATS_FILE_TMPDIR}/$(basename "${pod_config_template}").XXX")
    IMAGE="$large_image" NODE_NAME="$node" envsubst < "$pod_config_template" > "$pod_config"

    # Set a short CreateContainerRequest timeout in the annotation to fail to pull image in guest
    create_container_timeout=10
    set_metadata_annotation "$pod_config" \
        "io.katacontainers.config.runtime.create_container_timeout" \
        "${create_container_timeout}"

    # Enable dm-integrity in guest
    set_metadata_annotation "${pod_config}" \
        "io.katacontainers.config.hypervisor.kernel_params" \
        "agent.secure_storage_integrity=true"

    # Set annotation to pull image in guest
    set_metadata_annotation "${pod_config}" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    # For debug sake
    echo "Pod $pod_config file:"
    cat $pod_config

    # The pod should be failed because the image is too large to be pulled in the timeout
    assert_pod_fail "$pod_config"
    assert_logs_contain "$node" kata "$node_start_time" 'createContainer failed'
    assert_logs_contain "$node" kata "$node_start_time" 'timeout'
}

@test "Test we can pull a large image inside the guest with large createcontainer timeout" {
	[ "$(uname -m)" == "s390x" ] && skip "See: https://github.com/kata-containers/kata-containers/issues/10838"

    if [ "${KATA_HYPERVISOR}" = "qemu-coco-dev" ] && [ "${KBS_INGRESS}" = "aks" ]; then
        skip "skip this specific one due to issue https://github.com/kata-containers/kata-containers/issues/10299"
    fi
    storage_config=$(mktemp "${BATS_FILE_TMPDIR}/$(basename "${storage_config_template}").XXX")
    local_device=$(create_loop_device)
    LOCAL_DEVICE="$local_device" NODE_NAME="$node" envsubst < "$storage_config_template" > "$storage_config"

    # For debug sake
    echo "Trusted storage $storage_config file:"
    cat $storage_config

    # Create persistent volume and persistent volume claim
    retry_kubectl_apply $storage_config

    pod_config=$(mktemp "${BATS_FILE_TMPDIR}/$(basename "${pod_config_template}").XXX")
    IMAGE="$large_image" NODE_NAME="$node" envsubst < "$pod_config_template" > "$pod_config"

    # Set CreateContainerRequest timeout in the annotation to pull large image in guest
    create_container_timeout=120
    [ "${KATA_HYPERVISOR}" == "qemu-coco-dev" ] && create_container_timeout=600
    set_metadata_annotation "$pod_config" \
        "io.katacontainers.config.runtime.create_container_timeout" \
        "${create_container_timeout}"

    # Enable dm-integrity in guest
    set_metadata_annotation "${pod_config}" \
        "io.katacontainers.config.hypervisor.kernel_params" \
        "agent.secure_storage_integrity=true"

    # Set annotation to pull image in guest
    set_metadata_annotation "${pod_config}" \
        "io.containerd.cri.runtime-handler" \
        "kata-${KATA_HYPERVISOR}"

    # For debug sake
    echo "Pod $pod_config file:"
    cat $pod_config

    add_allow_all_policy_to_yaml "$pod_config"
    local wait_time=120
    [ "${KATA_HYPERVISOR}" == "qemu-coco-dev" ] && wait_time=600
    k8s_create_pod "$pod_config" "$wait_time"
}

teardown() {
    if ! is_confidential_runtime_class; then
        skip "Test not supported for ${KATA_HYPERVISOR}."
    fi

    [ "${SNAPSHOTTER:-}" = "nydus" ] || skip "None snapshotter was found but this test requires one"

    teardown_common "${node}" "${node_start_time:-}"
    kubectl delete --ignore-not-found pvc trusted-pvc
    kubectl delete --ignore-not-found pv trusted-block-pv
    kubectl delete --ignore-not-found storageclass local-storage
    cleanup_loop_device || true
}
