#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Regression test: a Guaranteed-QoS pod's vCPU threads must be pinned 1:1 to
# host CPUs when enable_vcpus_pinning=true and static_sandbox_resource_mgmt=true.
#
# With overhead_vcpus=0.5 the VM boots one more vCPU than the pod has CPUs
# (ceil(N+0.5)=N+1); the first N "CPU n/KVM" threads must be pinned 1:1 to
# distinct host CPUs, and the single overhead vCPU is allowed to float.
#
# IMPORTANT — what this test can and cannot validate:
#   The Kata runtime pins vCPU threads to the host cpuset that the kubelet's
#   static CPU manager assigns to the (Guaranteed) container.  When that
#   policy is NOT active, the container gets no exclusive cpuset:
#     * The Go runtime still pins because enable_numa=true gives it a
#       NUMA-derived host-CPU fallback.
#     * runtime-rs does NOT implement NUMA yet, so with no cpuset it has
#       nothing to pin to and correctly skips.
#   Therefore this test only exercises the pinning path on nodes whose
#   kubelet runs the *static* CPU manager policy; otherwise it skips.  The
#   no-cpuset case for runtime-rs will only pin once NUMA support lands.
#
# Prerequisites:
# * KATA_HYPERVISOR ships enable_vcpus_pinning=true (NVIDIA GPU configs).
# * The node's kubelet runs --cpu-manager-policy=static (test skips otherwise).
# * The bats runner executes directly on the node with sudo access to
#   /proc, crictl, and taskset — same requirement as k8s-nvidia-numa.bats.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

# Hypervisors that ship enable_vcpus_pinning = true by default.
# Covers both Go ("qemu-nvidia-gpu*") and runtime-rs ("*-runtime-rs") variants.
VCPU_PINNING_SUPPORTED_BY_DEFAULT=(
    "qemu-nvidia-gpu"
    "qemu-nvidia-gpu-snp"
    "qemu-nvidia-gpu-tdx"
    "qemu-nvidia-gpu-runtime-rs"
    "qemu-nvidia-gpu-snp-runtime-rs"
    "qemu-nvidia-gpu-tdx-runtime-rs"
)

# Number of exclusive integer CPUs requested by the pod.  The pod YAML must
# request the same value.
VCPU_PINNING_TEST_CPUS="${VCPU_PINNING_TEST_CPUS:-2}"

POD_NAME_PIN="vcpu-pinning-test"
POD_WAIT_TIMEOUT="${POD_WAIT_TIMEOUT:-120s}"

# Retry parameters for the host-side pinning poll.  The runtime pins vCPU
# threads shortly after sandbox creation; give it up to ~20 s.
HOST_PINNING_RETRIES="${HOST_PINNING_RETRIES:-20}"
HOST_PINNING_SLEEP="${HOST_PINNING_SLEEP:-1}"

setup() {
    setup_common || die "setup_common failed"

    # Only run on hypervisors that have pinning enabled by default.
    # shellcheck disable=SC2076
    if [[ ! " ${VCPU_PINNING_SUPPORTED_BY_DEFAULT[*]} " =~ " ${KATA_HYPERVISOR} " ]]; then
        skip "enable_vcpus_pinning not configured by default on ${KATA_HYPERVISOR}"
    fi

    yaml_file="${pod_config_dir}/pod-vcpu-pinning.yaml"
    set_node "${yaml_file}" "${node}"

    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
    add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
    auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

# ---------------------------------------------------------------------------
# Skip / host-side helpers
# ---------------------------------------------------------------------------

# cpu_manager_policy echoes the kubelet's CPU manager policyName for the
# target node (e.g. "static" or "none"), or "" if the state file is absent.
cpu_manager_policy() {
    local kubelet_data_dir state_file
    kubelet_data_dir="$(get_kubelet_data_dir)"
    state_file="${kubelet_data_dir}/cpu_manager_state"

    exec_host "${node}" "test -f '${state_file}'" >/dev/null 2>&1 || return 0
    exec_host "${node}" \
        "grep -oE '\"policyName\"[[:space:]]*:[[:space:]]*\"[^\"]*\"' '${state_file}' | head -1" \
        2>/dev/null | sed -E 's/.*:[[:space:]]*"([^"]*)"/\1/'
}

# get_qemu_pid_for_pin_pod resolves the running pod's sandbox via crictl
# and returns the QEMU PID.  Fails the test if either lookup is empty.
get_qemu_pid_for_pin_pod() {
    local sandbox_id qemu_pid
    sandbox_id=$(sudo crictl --runtime-endpoint unix:///run/containerd/containerd.sock \
        pods --name "${POD_NAME_PIN}" -q | head -1)
    [[ -n "${sandbox_id}" ]] || die "no sandbox id found for pod ${POD_NAME_PIN}"

    qemu_pid=$(sudo pgrep -f "qemu.*${sandbox_id}" | head -1)
    [[ -n "${qemu_pid}" ]] || die "no QEMU PID found for sandbox ${sandbox_id}"
    echo "${qemu_pid}"
}

# wait_for_vcpu_pinning <qemu_pid> <expected_pinned>
# Poll vcpu-pinning-check.sh until at least <expected_pinned> vCPU threads
# report single-CPU affinity, or HOST_PINNING_RETRIES is exhausted.
# Echoes the final script output regardless of convergence so callers can
# assert on the full picture.
wait_for_vcpu_pinning() {
    local qemu_pid="${1}" expected="${2}"
    local script="${BATS_TEST_DIRNAME}/vcpu-pinning-check.sh"
    local output pinned
    local attempt

    for ((attempt = 1; attempt <= HOST_PINNING_RETRIES; attempt++)); do
        output=$(sudo bash "${script}" "${qemu_pid}")
        pinned=$(echo "${output}" | grep -c '^pinned ' || true)
        if (( pinned >= expected )); then
            echo "${output}"
            return 0
        fi
        echo "# vCPU pinning attempt ${attempt}/${HOST_PINNING_RETRIES}: ${pinned}/${expected} threads pinned" >&2
        sleep "${HOST_PINNING_SLEEP}"
    done

    echo "${output}"
}

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

@test "vCPU pinning: Guaranteed pod vCPU threads are pinned 1:1 to host CPUs" {
    # The runtime can only pin to an exclusive cpuset that the kubelet's
    # static CPU manager assigns.  Without the static policy the container
    # gets no cpuset; Go would still pin via its NUMA-derived fallback, but
    # runtime-rs has no NUMA support and (correctly) cannot pin.  Skip here
    # so the test only asserts where pinning is actually expected to work.
    # This case will start passing for runtime-rs once NUMA lands.
    local cpu_policy
    cpu_policy="$(cpu_manager_policy)"
    if [[ "${cpu_policy}" != "static" ]]; then
        skip "kubelet CPU manager policy is '${cpu_policy:-none/absent}', need 'static' for an exclusive cpuset; runtime-rs pinning without one requires NUMA support (not yet implemented) — will pass once NUMA lands"
    fi

    # Deploy the Guaranteed-QoS pod and wait for it to be ready.
    kubectl apply -f "${yaml_file}"
    kubectl wait --for=condition=Ready --timeout="${POD_WAIT_TIMEOUT}" pod "${POD_NAME_PIN}"

    local qemu_pid
    qemu_pid=$(get_qemu_pid_for_pin_pod)
    echo "# QEMU PID: ${qemu_pid}"

    # Poll until the runtime has had time to pin the vCPU threads.
    local pinning_output
    pinning_output=$(wait_for_vcpu_pinning "${qemu_pid}" "${VCPU_PINNING_TEST_CPUS}")
    echo "# vCPU pinning check output:"
    echo "${pinning_output}" | while IFS= read -r line; do echo "#   ${line}"; done

    # --- Assertion 1: at least VCPU_PINNING_TEST_CPUS workload vCPUs are pinned ---
    #
    # Before the fix (runtime-rs + overhead_vcpus=0.5): pinned == 0 because
    # num_vcpus(N+1) != num_cpus(N) caused the runtime to skip pinning.
    # After the fix: the first N vCPUs (by index) are pinned 1:1.
    local pinned_count
    pinned_count=$(echo "${pinning_output}" | grep -c '^pinned ' || true)
    echo "# Pinned vCPU threads: ${pinned_count} (need >= ${VCPU_PINNING_TEST_CPUS})"
    (( pinned_count >= VCPU_PINNING_TEST_CPUS )) || \
        die "vCPU pinning broken: expected >= ${VCPU_PINNING_TEST_CPUS} pinned threads, got ${pinned_count}. Full output: ${pinning_output}"

    # --- Assertion 2: each pinned vCPU maps to a *distinct* host CPU ---
    #
    # Verifies true 1:1 pinning, not a degenerate "all pinned to CPU 0".
    # Extract the host CPU column (field 3) from "pinned <idx> <cpu>" lines.
    local pinned_cpus distinct_count
    pinned_cpus=$(echo "${pinning_output}" | awk '/^pinned / {print $3}')
    distinct_count=$(echo "${pinned_cpus}" | sort -u | wc -l)
    echo "# Distinct host CPUs used for pinning: ${distinct_count} (need ${pinned_count})"
    (( distinct_count == pinned_count )) || \
        die "vCPU pinning is not 1:1: ${pinned_count} pinned threads share only ${distinct_count} distinct host CPUs. Full output: ${pinning_output}"

    # --- Informational: report overhead vCPUs (index >= VCPU_PINNING_TEST_CPUS) ---
    #
    # With overhead_vcpus > 0 (e.g. 0.5 on runtime-rs GPU configs) the VM
    # boots one more vCPU than the pod has CPUs.  After the fix the excess
    # vCPU is expected to float — it is constrained by the sandbox cgroup
    # cpuset but is not 1:1 pinned.  That is correct behaviour.
    local floating_count
    floating_count=$(echo "${pinning_output}" | grep -c '^floating ' || true)
    if (( floating_count > 0 )); then
        echo "# ${floating_count} overhead vCPU thread(s) are floating (expected for overhead_vcpus > 0)"
    fi
}

teardown() {
    # Mirror the setup() skip so teardown does not run against state that
    # was never initialised (policy_settings_dir, yaml_file, …).
    # shellcheck disable=SC2076
    [[ " ${VCPU_PINNING_SUPPORTED_BY_DEFAULT[*]} " =~ " ${KATA_HYPERVISOR} " ]] || return 0

    echo "=== vCPU pinning test pod describe ==="
    kubectl describe pod "${POD_NAME_PIN}" || true

    kubectl delete pod "${POD_NAME_PIN}" --ignore-not-found --timeout=60s || true

    delete_tmp_policy_settings_dir "${policy_settings_dir}"
    teardown_common "${node}" "${node_start_time:-}"
}
