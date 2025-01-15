#!/usr/bin/env bats
# Copyright (c) 2025 Microsoft Corporation
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
    is_confidential_runtime_class || skip "Only supported for CoCo"
    [ "$(uname -m)" == "s390x" ] && skip "Not supported on s390x"

    setup_common
    get_pod_config_dir

    pod_name="trusted-ephemeral-data-storage"
    mountpoint="/mnt/temp-encrypted"
    capacity_bytes="10000000" # FIXME: Set to host fs capacity.

    yaml_file="${pod_config_dir}/pod-trusted-ephemeral-data-storage.yaml"
    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

    # The policy would only block container creation, so allow these
    # requests to make writing tests easier.
    allow_requests "${policy_settings_dir}" "ExecProcessRequest" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

    installed_expect=false
    if ! exec_host "${node}" which expect; then
        exec_host "${node}" apt-get install -y expect
        installed_expect=true
    fi

    copy_file_to_host "${pod_config_dir}/cryptsetup.exp" "${node}" "/tmp/cryptsetup.exp"
}

@test "Trusted ephemeral data storage" {
    kubectl apply -f "${yaml_file}"
    kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

    # With long device names, df adds line breaks by default, so we pass -P to prevent that.
    df="$(kubectl exec "${pod_name}" -- df -PT "${mountpoint}" | tail -1)"
    info "df output:"
    info "${df}"

    dm_device="$(echo "${df}" | awk '{print $1}')"
    fs_type="$(echo "${df}" | awk '{print $2}')"
    available_bytes="$(echo "${df}" | awk '{print $5}')"

    # The output of the cryptsetup command will contain something like this:
    #
    #   /dev/mapper/encrypted_disk_N6PxO is active and is in use.
    #     type:    LUKS2
    #     cipher:  aes-xts-plain64
    #     keysize: 768 bits
    #     key location: keyring
    #     integrity: hmac(sha256)
    #     integrity keysize: 256 bits
    #     device:  /dev/vda
    #     sector size:  4096
    #     offset:  0 sectors
    #     size:    2031880 sectors
    #     mode:    read/write
    pod_id=$(exec_host "${node}" crictl pods -q --name "^${pod_name}$")
    crypt_status="$(exec_host "${node}" expect /tmp/cryptsetup.exp "${pod_id}" "${dm_device}")"
    info "cryptsetup status output:"
    info "${crypt_status}"

    # Check filesystem type and capacity.

    [[ "${fs_type}" == "ext4" ]]
    # Allow FS and encryption metadata to take up to 15% of storage.
    (( available_bytes >= capacity_bytes * 85 / 100 ))

    # Check encryption settings.

    grep -q "${dm_device} is active and is in use" <<< "${crypt_status}"
    grep -Eq "type: +LUKS2" <<< "${crypt_status}"
    grep -Eq "cipher: +aes-xts-plain64" <<< "${crypt_status}"
    grep -Eq "integrity: +hmac\(sha256\)" <<< "${crypt_status}"

    # Check I/O.

    kubectl exec "${pod_name}" -- sh -c "echo foo > "${mountpoint}/foo.txt""
    [[ "$(kubectl exec "${pod_name}" -- cat "${mountpoint}/foo.txt")" == "foo" ]]
}

teardown() {
    is_confidential_runtime_class || skip "Only supported for CoCo"
    [ "$(uname -m)" == "s390x" ] && skip "Not supported on s390x"

    exec_host "${node}" rm -f /tmp/cryptsetup.exp

    if [ "${installed_expect}" = true ]; then
        exec_host "${node}" bash -c "apt-get autoremove -y expect || true"
    fi

    confidential_teardown_common "${node}" "${node_start_time:-}"
}
