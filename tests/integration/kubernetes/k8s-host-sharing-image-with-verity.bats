#!/usr/bin/env bats
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

image_unsigned_unprotected_for_sharing="quay.io/sjenning/nginx:1.15-alpine"


setup() {
    [[ "${PULL_TYPE}" =~ "host-share" ]] || skip "Test only working for sharing image with verity on the host"
    setup_common
}

@test "Test can pull an image as a raw block disk image to guest with dm-verity" {
    if [ "$SNAPSHOTTER" = "nydus" ]; then
        pod_config="$(new_pod_config "${image_unsigned_unprotected_for_sharing}" "kata-${KATA_HYPERVISOR}")"
        echo $pod_config
        create_test_pod
    fi
}

@test "Test can create two pods with pulling the image only once with dm-verity enabled" {
    if [ "$SNAPSHOTTER" = "nydus" ]; then
        pod_config="$(new_pod_config "$image_unsigned_unprotected_for_sharing" "1")"
        echo $pod_config
        create_test_pod
        pod_config="$(new_pod_config "$image_unsigned_unprotected_for_sharing" "2")"
        echo $pod_config
        create_test_pod

        pull_count=$(journalctl -t containerd --since "$test_start_date" -g "PullImage \"$image_unsigned_protected\" with snapshotter nydus" | wc -l)
        [ ${pull_count} -eq 1 ]
    fi
}

teardown() {
    [[ "${PULL_TYPE}" =~ "host-share" ]] || skip "Test only working for sharing image with verity on the host"

    kubectl describe -f "${pod_config}"
    kubectl delete -f "${pod_config}"
}