#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-nvidia-gpu-runtime-rs}"

export POD_NAME_DCGM="nvidia-dcgm-exporter"

# dcgm-exporter's built-in listen address. NVRC passes it only -k and -f, so the
# default stands, and the guest shares the pod's network namespace: whatever the
# exporter binds inside the VM answers on the pod IP.
DCGM_EXPORTER_PORT="${DCGM_EXPORTER_PORT:-9400}"

POD_WAIT_TIMEOUT="${POD_WAIT_TIMEOUT:-300s}"

# The exporter starts with the sandbox, so it is normally listening well before
# the pod reports ready. The budget covers nv-hostengine still enumerating GPUs.
METRICS_TIMEOUT="${METRICS_TIMEOUT:-60}"

# NVRC keeps nv-hostengine and dcgm-exporter switched off until the guest is
# asked for them, and the request travels on the kernel command line.
#
# It has to arrive through a config.d/ drop-in rather than the pod annotation
# the CoCo tests use: the NVIDIA GPU shims ship an empty enable_annotations
# allowlist (DEFENABLEANNOTATIONS_NVIDIA), so a hypervisor annotation is
# refused before it can reach the guest.
#
# The runtime merges drop-ins in alphabetical order and a string value replaces
# rather than extends the one below it, so this fragment has to carry the whole
# kernel_params line. Read back whichever drop-in currently owns it -- the
# suite's own 90-nvrc-trace.toml, usually -- and fall back to the base config.
enable_dcgm_via_dropin() {
    local config_dir params local_dropin

    config_dir="$(get_kata_runtime_config_dir "${node}")" ||
        die "no Kata runtime config dir for ${KATA_HYPERVISOR}"

    params="$(exec_host "${node}" "
        params=
        for f in ${config_dir}/config.d/*.toml; do
            [ -f \"\${f}\" ] || continue
            v=\$(sed -n 's/^[[:space:]]*kernel_params[[:space:]]*=[[:space:]]*\"\(.*\)\"[[:space:]]*\$/\1/p' \"\${f}\" | tail -1)
            [ -n \"\${v}\" ] && params=\${v}
        done
        [ -n \"\${params}\" ] || params=\$(sed -n 's/^[[:space:]]*kernel_params[[:space:]]*=[[:space:]]*\"\(.*\)\"[[:space:]]*\$/\1/p' \\
            ${config_dir}/configuration-${KATA_HYPERVISOR}.toml | tail -1)
        printf '%s' \"\${params}\"
    ")"

    # Every shim this suite supports is QEMU-backed, which is the same
    # assumption run_kubernetes_nv_tests.sh makes when it enables NVRC tracing.
    local_dropin="${BATS_FILE_TMPDIR}/95-dcgm-test.toml"
    {
        echo "[hypervisor.qemu]"
        echo "kernel_params = \"${params} nvrc.dcgm=on\""
    } > "${local_dropin}"

    runtime_config_dropin="$(set_kata_runtime_config_dropin_file "${node}" "${local_dropin}")" ||
        die "failed to write the dcgm drop-in for ${KATA_HYPERVISOR}"
    export runtime_config_dropin

    echo "# Wrote drop-in ${runtime_config_dropin}:" >&3
    sed 's/^/# /' "${local_dropin}" >&3
}

# Scrape the exporter, retrying until it answers, and print the payload.
#
# A successful scrape is already evidence that the counter CSV shipped in the
# image: dcgm-exporter refuses to start when the file named by -f is missing.
scrape_dcgm_metrics() {
    local pod_ip
    pod_ip="$(kubectl get pod "${POD_NAME_DCGM}" -o jsonpath='{.status.podIP}')"
    [[ -n "${pod_ip}" ]] || die "${POD_NAME_DCGM}: no pod IP"

    local url="http://${pod_ip}:${DCGM_EXPORTER_PORT}/metrics"
    local deadline=$((SECONDS + METRICS_TIMEOUT))
    local body=""

    while ((SECONDS < deadline)); do
        if body="$(curl -sf --max-time 10 "${url}")"; then
            echo "${body}"
            return 0
        fi
        sleep 5
    done

    die "${POD_NAME_DCGM}: no metrics from ${url} within ${METRICS_TIMEOUT}s"
}

setup_file() {
    setup_common || die "setup_common failed"

    export POD_YAML_IN="${pod_config_dir}/${POD_NAME_DCGM}.yaml.in"
    export POD_YAML="${pod_config_dir}/${POD_NAME_DCGM}.yaml"
    envsubst < "${POD_YAML_IN}" > "${POD_YAML}"

    export policy_settings_dir
    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
    add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
    auto_generate_policy "${policy_settings_dir}" "${POD_YAML}"

    # Must precede the pod: the sandbox reads the drop-in when it boots.
    enable_dcgm_via_dropin

    kubectl apply -f "${POD_YAML}"
    kubectl wait --for=condition=Ready --timeout="${POD_WAIT_TIMEOUT}" pod "${POD_NAME_DCGM}"

    # BATS_TEST_COMPLETED is per-test and remains empty in teardown_file.
    # Persist file-level state so success does not trigger journal dumps.
    touch "${BATS_FILE_TMPDIR}/setup-file-completed"
}

teardown() {
    if [[ "${BATS_TEST_COMPLETED:-}" != "1" && -z "${BATS_TEST_SKIPPED:-}" ]]; then
        touch "${BATS_FILE_TMPDIR}/test-failed"
    fi
}

@test "dcgm-exporter serves GPU metrics" {
    run scrape_dcgm_metrics
    [[ "${status}" -eq 0 ]]

    # Count series lines rather than matching the name anywhere in the body: an
    # exporter that enumerated nothing still emits the HELP and TYPE comments.
    local series
    series="$(grep -c '^DCGM_FI_' <<<"${output}" || true)"
    [[ "${series}" -gt 0 ]]

    echo "# DCGM_FI_ series: ${series}" >&3
}

@test "dcgm-exporter metrics carry GPU identity" {
    run scrape_dcgm_metrics
    [[ "${status}" -eq 0 ]]

    # Labels come from DCGM's device enumeration, so their presence means the
    # exporter reached the passthrough GPU rather than publishing an empty
    # scrape with no devices behind it.
    [[ "${output}" =~ gpu=\" ]]
    [[ "${output}" =~ UUID=\" ]]

    echo "# $(grep -m1 '^DCGM_FI_' <<<"${output}")" >&3
}

teardown_file() {
    kubectl describe pod "${POD_NAME_DCGM}" || true

    # Before anything that might fail: a leaked drop-in would leave DCGM on for
    # every pod that lands on this node afterwards.
    remove_kata_runtime_config_dropin_file "${node}" "${runtime_config_dropin:-}" || true

    delete_tmp_policy_settings_dir "${policy_settings_dir}"

    [[ -f "${POD_YAML}" ]] && kubectl delete -f "${POD_YAML}" --ignore-not-found=true

    # The exporter logs to the guest console, not to the container, so NVRC's
    # output on the node journal is where a failed scrape gets explained.
    local bats_test_completed=1
    if [[ ! -f "${BATS_FILE_TMPDIR}/setup-file-completed" || -f "${BATS_FILE_TMPDIR}/test-failed" ]]; then
        bats_test_completed=
    fi
    print_node_journal_since_test_start "${node}" "${node_start_time:-}" "${bats_test_completed}"
}
