#!/usr/bin/env bats
# Copyright (c) 2025 Microsoft Corporation
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
    is_confidential_runtime_class || skip "Only supported for CoCo"
    [[ "${KATA_HYPERVISOR}" == *-runtime-rs ]] && skip "Not supported with runtime-rs"

    setup_common
    get_pod_config_dir

    pod_name="trusted-ephemeral-data-storage"
    mountpoint="/mnt/temp-encrypted"

    host_df="$(exec_host "${node}" df -PT -B1 "$(get_kubelet_data_dir)" | tail -n +2)"
    info "host_df output:"
    info "${host_df}"
    host_cap_bytes="$(echo "${host_df}" | awk '{print $3}')"

    yaml_file="${pod_config_dir}/pod-trusted-ephemeral-data-storage.yaml"
    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

    # The policy would only block container creation, so allow these
    # requests to make writing tests easier.
    allow_requests "${policy_settings_dir}" "ExecProcessRequest" "ReadStreamRequest"
    auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Trusted ephemeral data storage" {
    kubectl apply -f "${yaml_file}"
    kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

    # With long device names, df adds line breaks by default, so we pass -P to prevent that.
    emptydir_df="$(kubectl exec "${pod_name}" -- df -PT -B1 "${mountpoint}" | tail -n +2)"
    info "emptydir_df output:"
    info "${emptydir_df}"

    dm_device="$(echo "${emptydir_df}" | awk '{print $1}')"
    fs_type="$(echo "${emptydir_df}" | awk '{print $2}')"
    emptydir_cap_bytes="$(echo "${emptydir_df}" | awk '{print $3}')"
    emptydir_avail_bytes="$(echo "${emptydir_df}" | awk '{print $5}')"

    # The output of the cryptsetup command will contain something like this:
    #
    #   /dev/mapper/741ed4bf-3073-49ed-9b7a-d6fa7cce0db1 is active and is in use.
    #     type:    n/a
    #     cipher:  aes-xts-plain
    #     keysize: 768 bits
    #     key location: keyring
    #     integrity: hmac(sha256)
    #     integrity keysize: 256 bits
    #     integrity tag size: 32 bytes
    #     device:  /dev/sdd
    #     sector size:  4096
    #     offset:  0 sectors
    #     size:    300052568 sectors
    #     mode:    read/write
    crypt_status="$(kubectl exec "${pod_name}" -- cryptsetup status "${dm_device}")"
    info "cryptsetup status output:"
    info "${crypt_status}"

    # Check filesystem type and capacity.

    [[ "${fs_type}" == "ext4" ]]
    # Allow up to 4% metadata overhead.
    (( emptydir_cap_bytes >= host_cap_bytes * 96 / 100 ))
    # Allow up to 10% metadata overhead.
    (( emptydir_avail_bytes >= host_cap_bytes * 90 / 100 ))

    # Check encryption settings.
    grep -q "${dm_device} is active and is in use" <<< "${crypt_status}"
    grep -Eq "type: +n/a" <<< "${crypt_status}" # The LUKS header is detached.
    grep -Eq "cipher: +aes-xts-plain" <<< "${crypt_status}"
    grep -Eq "integrity: +hmac\(sha256\)" <<< "${crypt_status}"

    # Check I/O.

    kubectl exec "${pod_name}" -- sh -c "echo foo > '${mountpoint}/foo.txt'"
    [[ "$(kubectl exec "${pod_name}" -- cat "${mountpoint}/foo.txt")" == "foo" ]]
}

teardown() {
    is_confidential_runtime_class || skip "Only supported for CoCo"
    [[ "${KATA_HYPERVISOR}" == *-runtime-rs ]] && skip "Not supported with runtime-rs"

    confidential_teardown_common "${node}" "${node_start_time:-}"
}
