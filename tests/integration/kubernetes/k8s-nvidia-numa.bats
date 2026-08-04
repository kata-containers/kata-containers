#!/usr/bin/env bats
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# NUMA topology and vCPU pinning verification tests for Kata Containers.
#
# Seven tests cover the main paths in the runtime's NUMA logic:
#   1. Multi-node sandbox: a workload that does NOT fit in a single host
#      NUMA node should be balanced across host nodes — the guest sees
#      multiple NUMA nodes with even vCPU/memory distribution and host
#      vCPU pinning is balanced as well.
#   2. Right-sized single-node sandbox: a workload that DOES fit in a
#      single host NUMA node should be collapsed to one node — the guest
#      sees exactly one NUMA node with all vCPUs in it AND all host
#      QEMU vCPU threads are pinned to that one host NUMA node.
#   3. GPU passthrough (VFIO), multi-node: when a GPU is attached via
#      VFIO and the workload spans every host NUMA node, the runtime
#      creates pxb-pcie bridges and the guest GPU reports the same NUMA
#      node as the host GPU.
#   4. GPU passthrough (VFIO), right-sized single-node: when a small
#      workload + GPU fits on a single host NUMA node, the runtime
#      collapses the topology to the GPU's host NUMA node (memory and
#      vCPUs land on the same node as the GPU, not just any fitting node).
#   5. Explicit numa_mapping in the runtime TOML: when the user pins the
#      guest topology to a specific host node via numa_mapping = ["1"],
#      maybeRightSizeAutoNUMA() must be a no-op and buildNUMATopology()
#      must propagate the binding (memory + vCPU pinning land on the
#      chosen host node, regardless of how small the workload is).
#   6. Two sandboxes side by side: neither may claim a host CPU the other
#      already claimed, whatever the host NUMA node count.
#   7. Two guest NUMA nodes on one host node (numa_mapping = ["0", "0"]):
#      a single sandbox must not put two of its own vCPUs on one host CPU.
#
# Tests 6 and 7 guard kata-containers#13539 and run on single-NUMA hosts
# too, since that is where the reported reproduction lives.
#
# The NVIDIA configurations ship with NUMA and vCPU pinning turned off,
# so every test here turns both on through a config.d/ drop-in before it
# starts a sandbox.  That keeps the suite running against the shipped
# configuration whatever its defaults are, and it makes each test state
# the feature it exercises instead of inheriting it.
#
# Guest-side checks use the quay.io/kata-containers/numa container image
# which reads sysfs and prints results to stdout.  The bats test reads
# the output via "kubectl logs" — no kubectl exec, no CoCo policy
# overrides needed.
#
# WARNING: The host-side pinning check runs numa-pinning-check.sh directly
# on the host (not inside a container).  This requires the bats runner to
# execute on the k8s node with privileged access to /proc, /sys and crictl.
# If the test environment changes so that bats no longer runs on the node,
# these calls must be reworked to use exec_host or equivalent.
#
# The host-side checks assert on where the vCPU threads may run, not on how
# narrow their affinity is: the runtime gives each vCPU a host CPU of its own
# only when the sandbox owns its cpuset, and confines the threads to their
# NUMA node otherwise.  What must hold in either case is that the threads
# stay on the expected host node(s) and that no two of them are pinned to the
# same host CPU.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"

export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu-nvidia-gpu-snp}"

# Hypervisors that implement NUMA, and can therefore have it enabled by the
# drop-in these tests write.  runtime-rs does not implement NUMA; non-QEMU
# hypervisors lack support.
NUMA_SUPPORTED=(
    "qemu-nvidia-cpu"
    "qemu-nvidia-gpu"
    "qemu-nvidia-gpu-snp"
    "qemu-nvidia-gpu-tdx"
)

# Hypervisors that support GPU passthrough (VFIO).  qemu-nvidia-cpu is a CPU-only
# NVIDIA class that deliberately disables passthrough, so the GPU NUMA tests
# must skip there even on a GPU-equipped host (they'd otherwise fail rather
# than skip once nvidia.com/pgpu resources are present).
NUMA_GPU_SUPPORTED=(
    "qemu-nvidia-gpu"
    "qemu-nvidia-gpu-snp"
    "qemu-nvidia-gpu-tdx"
)

# Multi-node test: large enough to span every host NUMA node.
NUMA_TEST_VCPUS_LARGE="${NUMA_TEST_VCPUS_LARGE:-64}"
NUMA_TEST_MEMORY_LARGE="${NUMA_TEST_MEMORY_LARGE:-64Gi}"

# Right-sizing test: small enough to fit in a single host NUMA node on
# any reasonable production-class server.
NUMA_TEST_VCPUS_SMALL="${NUMA_TEST_VCPUS_SMALL:-4}"
NUMA_TEST_MEMORY_SMALL="${NUMA_TEST_MEMORY_SMALL:-4Gi}"

# GPU test: same sizing as the large test, plus a GPU.
NUMA_TEST_VCPUS_GPU="${NUMA_TEST_VCPUS_GPU:-64}"
NUMA_TEST_MEMORY_GPU="${NUMA_TEST_MEMORY_GPU:-64Gi}"

# Small GPU test: fits in a single host NUMA node, exercises the
# right-sizing path with VFIO (sandbox should land on the GPU's host
# NUMA node, not just any node that fits).
NUMA_TEST_VCPUS_GPU_SMALL="${NUMA_TEST_VCPUS_GPU_SMALL:-4}"
NUMA_TEST_MEMORY_GPU_SMALL="${NUMA_TEST_MEMORY_GPU_SMALL:-4Gi}"

# Two-sandbox test: small enough to run a pair of them side by side.
NUMA_TEST_VCPUS_PAIR="${NUMA_TEST_VCPUS_PAIR:-2}"
NUMA_TEST_MEMORY_PAIR="${NUMA_TEST_MEMORY_PAIR:-1Gi}"

export POD_NAME_NUMA="numa-topology-test"
POD_NAME_NUMA_B="numa-topology-test-b"
POD_NAME_NUMA_GPU="numa-topology-gpu-test"

POD_WAIT_TIMEOUT=${POD_WAIT_TIMEOUT:-600s}
export POD_WAIT_TIMEOUT

HOST_PINNING_RETRIES=20
HOST_PINNING_SLEEP=0.5

setup() {
    setup_common || die "setup_common failed"

    pod_yaml_in="${pod_config_dir}/${POD_NAME_NUMA}.yaml.in"
    pod_yaml="${pod_config_dir}/${POD_NAME_NUMA}.yaml"
    pod_yaml_b="${pod_config_dir}/${POD_NAME_NUMA_B}.yaml"

    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
    add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
}

# -----------------------------------------------------------------------------
# Skip / topology helpers
# -----------------------------------------------------------------------------

# numa_unsupported_reason returns a non-empty reason on stdout when the
# hypervisor under test does not implement NUMA, and nothing when it does.
# Tests that hold on any host NUMA topology use this one.
# Callers must invoke `skip` themselves — bats `skip` inside command
# substitution does not propagate.
numa_unsupported_reason() {
    # shellcheck disable=SC2076
    if [[ ! " ${NUMA_SUPPORTED[*]} " =~ " ${KATA_HYPERVISOR} " ]]; then
        echo "NUMA not supported on ${KATA_HYPERVISOR}"
    fi
}

# numa_skip_reason extends numa_unsupported_reason with the >= 2 host NUMA
# nodes a test needs to tell node-local placement from node-remote.
numa_skip_reason() {
    local reason
    reason=$(numa_unsupported_reason)
    if [[ -n "${reason}" ]]; then
        echo "${reason}"
        return 0
    fi
    local nodes
    nodes=$(host_numa_node_count)
    if [[ "${nodes}" -lt 2 ]]; then
        echo "Host has only ${nodes} NUMA node(s), need >= 2 for this test"
    fi
}

# host_numa_node_count echoes the number of NUMA nodes on the host.
# WARNING: numactl runs directly on the host, not via exec_host.
host_numa_node_count() {
    numactl --hardware | grep -oP 'available:\s+\K\d+'
}

# -----------------------------------------------------------------------------
# Pod lifecycle helpers
# -----------------------------------------------------------------------------

# render_pod renders the pod yaml with the given vCPU and memory limits
# and runs auto_generate_policy against it.  Each @test calls this with
# its own sizing so the same template can serve multiple scenarios.
render_pod() {
    local vcpus="${1}" memory="${2}"
    NUMA_TEST_VCPUS="${vcpus}" NUMA_TEST_MEMORY="${memory}" \
        envsubst < "${pod_yaml_in}" > "${pod_yaml}"
    auto_generate_policy "${policy_settings_dir}" "${pod_yaml}"
}

# deploy_and_get_guest_logs renders, applies, waits for Ready, then
# echoes the pod's stdout (the test image prints NUMA topology then
# sleeps).  The brief sleep gives the entrypoint time to print before
# we read.
deploy_and_get_guest_logs() {
    local vcpus="${1}" memory="${2}"
    render_pod "${vcpus}" "${memory}"
    kubectl apply -f "${pod_yaml}"
    kubectl wait --for=condition=Ready --timeout="${POD_WAIT_TIMEOUT}" pod "${POD_NAME_NUMA}"
    sleep 2
    kubectl logs "${POD_NAME_NUMA}"
}

# deploy_second_pod <vcpus> <memory> <node> renders the same template under a
# second name and runs it on <node>, so both sandboxes compete for the same
# host CPUs — otherwise there is nothing for them to collide over. Waits for
# the pod to be Ready.
deploy_second_pod() {
    local vcpus="${1}" memory="${2}" target_node="${3}"

    POD_NAME_NUMA="${POD_NAME_NUMA_B}" NUMA_TEST_VCPUS="${vcpus}" \
        NUMA_TEST_MEMORY="${memory}" \
        envsubst < "${pod_yaml_in}" > "${pod_yaml_b}"
    set_node "${pod_yaml_b}" "${target_node}"
    auto_generate_policy "${policy_settings_dir}" "${pod_yaml_b}"

    kubectl apply -f "${pod_yaml_b}"
    kubectl wait --for=condition=Ready --timeout="${POD_WAIT_TIMEOUT}" pod "${POD_NAME_NUMA_B}"
}

# -----------------------------------------------------------------------------
# Guest-log parsers (operate on stdout from the test container)
# -----------------------------------------------------------------------------

# guest_online_count parses a "numa_online: <value>" payload (e.g. "0",
# "0-1", "0-7") and echoes the number of online NUMA nodes it implies.
guest_online_count() {
    local online="${1}"
    if [[ "${online}" =~ ^([0-9]+)-([0-9]+)$ ]]; then
        echo $(( ${BASH_REMATCH[2]} - ${BASH_REMATCH[1]} + 1 ))
    elif [[ "${online}" =~ ^[0-9]+$ ]]; then
        echo 1
    else
        die "Unexpected format for guest NUMA online nodes: ${online}"
    fi
}

# guest_field <logs> <field>
# Echoes the value following "<field>:" in <logs>.  E.g.
#   guest_field "$logs" numa_online -> "0-1"
guest_field() {
    echo "${1}" | grep -oP "${2}:\s*\K\S+"
}

# guest_per_node_values <logs> <suffix>
# Emits one value per line for "node\d+<suffix>: <value>" entries
# (e.g. _cpus or _mem_kb).  Suitable for `mapfile -t`.
guest_per_node_values() {
    echo "${1}" | grep -oP "node\d+${2}:\s*\K\d+"
}

# -----------------------------------------------------------------------------
# Host-side pinning helpers
# -----------------------------------------------------------------------------

# pinning_thread_total sums the per-bucket counts in numa-pinning-check.sh
# output ("nodeN: <count>" lines) and echoes the total.
pinning_thread_total() {
    echo "${1}" | awk -F: '/^node[0-9]+:/ {sum+=$2} END {print sum+0}'
}

# assert_no_shared_host_cpus <check_output>
# Fails when two vCPU threads are pinned to the same host CPU.  Both then
# sustain roughly half the throughput of a thread that owns its CPU, while
# other CPUs stay idle (kata-containers#13539).
assert_no_shared_host_cpus() {
    local threads cpus
    threads=$(check_output_field "${1}" single_cpu_threads)
    cpus=$(check_output_field "${1}" distinct_single_cpus)
    echo "# vCPU threads pinned to one CPU: ${threads:-0} on ${cpus:-0} distinct host CPUs"
    [[ "${threads:-0}" -eq "${cpus:-0}" ]] \
        || die "${threads} vCPU threads pinned to only ${cpus} host CPUs: ${1}"
}

# node_cpu_manager_policy <node>
# Echoes the kubelet's cpuManagerPolicy on <node>, or an empty string when the
# kubelet configuration cannot be read — the node may not allow the configz
# endpoint.  Never fails, so that a caller running under `set -e` reaches its
# own handling of an unknown policy.
node_cpu_manager_policy() {
    local config
    config=$(kubectl get --raw "/api/v1/nodes/${1}/proxy/configz" 2>/dev/null) \
        || return 0
    echo "${config}" | grep -o '"cpuManagerPolicy":"[^"]*"' | cut -d'"' -f4 \
        || return 0
}

# assert_pins_match_exclusive_cpus <pod_name> <check_output> <pod_cpu_limit>
# The kubelet reserves a Guaranteed pod's whole CPU limit under
# cpuManagerPolicy=static and nothing at all under none, so a sandbox pins
# exactly that many vCPU threads to a host CPU each under static, and none at
# all under none.  A count in between means the runtime handed out CPUs it does
# not hold, or left vCPUs unpinned that the cpuset could back.  Checking only
# that no two threads share a CPU would pass on a node reserving CPUs even if
# nothing were pinned at all.
assert_pins_match_exclusive_cpus() {
    local pod_name="${1}" output="${2}" expected="${3}"
    local node policy threads
    threads=$(check_output_field "${output}" single_cpu_threads)
    threads="${threads:-0}"

    node=$(kubectl get pod "${pod_name}" -o jsonpath='{.spec.nodeName}') \
        || die "cannot tell which node pod ${pod_name} runs on"
    [[ -n "${node}" ]] || die "pod ${pod_name} has no node assigned"

    policy=$(node_cpu_manager_policy "${node}") || policy=""
    echo "# cpuManagerPolicy on ${node}: ${policy:-<unreadable>}"

    case "${policy}" in
        static)
            [[ "${threads}" -eq "${expected}" ]] \
                || die "cpuManagerPolicy=static reserves ${expected} CPUs for the pod, but ${threads} vCPU threads hold a host CPU of their own: ${output}"
            ;;
        none)
            [[ "${threads}" -eq 0 ]] \
                || die "cpuManagerPolicy=none reserves no CPU for the pod, yet ${threads} vCPU threads hold one of their own: ${output}"
            ;;
        *)
            echo "# Skipping the pin count check: the kubelet's cpuManagerPolicy is unknown"
            ;;
    esac
}

# check_output_field <check_output> <field>
# Echoes the value of a "<field>: <value>" line of numa-pinning-check.sh
# output, empty when the field is absent.  awk rather than grep so a missing
# field yields an empty value instead of failing the calling test.
check_output_field() {
    echo "${1}" | awk -v field="${2}:" '$1 == field {print $2}'
}

# pinned_cpu_list <check_output>
# Echoes the comma-separated host CPUs the sandbox claimed for a single vCPU
# thread each, empty when it claimed none.
pinned_cpu_list() {
    check_output_field "${1}" pinned_cpus
}

# wait_for_host_pinning <qemu_pid> <expected_vcpus>
# Polls numa-pinning-check.sh until at least <expected_vcpus> threads
# are confined to a single host NUMA node, or until HOST_PINNING_RETRIES
# is exhausted.  Echoes the final script output regardless of whether
# convergence was reached, so callers can inspect/assert on the bucket
# distribution.
wait_for_host_pinning() {
    local qemu_pid="${1}" expected="${2}"
    local script="${BATS_TEST_DIRNAME}/numa-pinning-check.sh"
    local output total
    local attempt
    for ((attempt = 1; attempt <= HOST_PINNING_RETRIES; attempt++)); do
        output=$(sudo bash "${script}" "${qemu_pid}")
        total=$(pinning_thread_total "${output}")
        if (( total >= expected )); then
            echo "${output}"
            return 0
        fi
        echo "# Host pinning attempt ${attempt}/${HOST_PINNING_RETRIES}: ${total}/${expected} threads placed" >&2
        sleep "${HOST_PINNING_SLEEP}"
    done
    echo "${output}"
}

# minmax_diff <values...>
# Echoes (max - min) for the given non-empty integer list.
minmax_diff() {
    local lo=$1 hi=$1 v
    shift
    for v in "$@"; do
        (( v > hi )) && hi=$v
        (( v < lo )) && lo=$v
    done
    echo $((hi - lo))
}

# get_qemu_cmdline <qemu_pid>
# Reads the QEMU process command line from /proc, replacing null bytes
# with spaces.  Runs directly on the host via sudo.
get_qemu_cmdline() {
    sudo cat "/proc/${1}/cmdline" | tr '\0' ' '
}

# host_has_pgpu returns 0 if the node has allocatable nvidia.com/pgpu
# resources, 1 otherwise.
host_has_pgpu() {
    local count
    count=$(kubectl get nodes -o jsonpath='{.items[*].status.allocatable.nvidia\.com/pgpu}' 2>/dev/null)
    [[ -n "${count}" && "${count}" -gt 0 ]] 2>/dev/null
}

# gpu_numa_skip_reason extends numa_skip_reason with a check for GPU
# availability.
gpu_numa_skip_reason() {
    local reason
    reason=$(numa_skip_reason)
    if [[ -n "${reason}" ]]; then
        echo "${reason}"
        return 0
    fi
    # shellcheck disable=SC2076
    if [[ ! " ${NUMA_GPU_SUPPORTED[*]} " =~ " ${KATA_HYPERVISOR} " ]]; then
        echo "GPU passthrough not supported on ${KATA_HYPERVISOR} (CPU-only NVIDIA class)"
        return 0
    fi
    if ! host_has_pgpu; then
        echo "No nvidia.com/pgpu resources available on the cluster"
    fi
}

# -----------------------------------------------------------------------------
# NUMA config helpers (drop-in based)
# -----------------------------------------------------------------------------
#
# Both kata-runtime (Go) and runtime-rs (Rust) read TOML fragments from a
# `config.d/` directory next to the active configuration-<shim>.toml file
# and merge them into the loaded config on every sandbox start.  These
# helpers drop in a single override fragment so the main config file is
# never edited — teardown just deletes the fragment.

# kata_hypervisor_section echoes the [hypervisor.X] header from the active
# config so the drop-in fragment targets the right table.  Discovering it
# at runtime keeps us hypervisor-agnostic (qemu / clh / firecracker / ...).
kata_hypervisor_section() {
    local cfg
    cfg=$(get_kata_runtime_config_file "${node}") || \
        die "no Kata runtime config file for ${KATA_HYPERVISOR}"

    local section
    section=$(exec_host "${node}" "grep -oE '^\\[hypervisor\\.[a-z0-9_-]+\\]' '${cfg}' | head -1")
    [[ -n "${section}" ]] || die "no [hypervisor.X] section in ${cfg}"
    echo "${section}"
}

# patch_kata_numa_config [numa_mapping_toml_value]
# Writes a config.d/ drop-in that enables NUMA and vCPU pinning, optionally
# binding the guest topology with numa_mapping = <toml_value> (e.g. '["1"]',
# '["0-1","2-3"]').  Every test needs this because the shipped NVIDIA
# configurations keep both features off.  Records the file path in
# KATA_NUMA_DROPIN_PATH so teardown() can remove it.  No restart needed —
# the next sandbox start picks it up.
#
# enable_numa and numa_mapping belong to the hypervisor table while
# enable_vcpus_pinning belongs to [runtime], so the fragment carries both
# tables.
patch_kata_numa_config() {
    local mapping="${1:-}"
    local local_dropin section
    section=$(kata_hypervisor_section)

    local_dropin="${BATS_FILE_TMPDIR}/99-numa-test.toml"
    {
        echo "${section}"
        echo "enable_numa = true"
        [[ -z "${mapping}" ]] || echo "numa_mapping = ${mapping}"
        echo
        echo "[runtime]"
        echo "enable_vcpus_pinning = true"
    } > "${local_dropin}"

    KATA_NUMA_DROPIN_PATH="$(set_kata_runtime_config_dropin_file \
        "${node}" \
        "${local_dropin}")" || \
        die "failed to write Kata runtime config drop-in for ${KATA_HYPERVISOR}"
    export KATA_NUMA_DROPIN_PATH

    echo "# Wrote drop-in ${KATA_NUMA_DROPIN_PATH}:"
    sed 's/^/# /' "${local_dropin}"
}

# restore_kata_numa_config removes the drop-in file written by
# patch_kata_numa_config (no-op if nothing was patched).
restore_kata_numa_config() {
    remove_kata_runtime_config_dropin_file "${node}" "${KATA_NUMA_DROPIN_PATH:-}" || return 1
    unset KATA_NUMA_DROPIN_PATH
}

# extract_vfio_host_bdf <qemu_cmdline>
# Returns the host PCI BDF of the first vfio-pci device passed through.
# E.g. "vfio-pci,host=0000:41:00.0,..." -> "0000:41:00.0".
extract_vfio_host_bdf() {
    echo "${1}" | grep -oP 'vfio-pci,host=\K[0-9a-fA-F:.]+' | head -1
}

# host_gpu_numa <host_bdf>
# Returns the NUMA node ID of a host PCI device from sysfs.
# Reads /sys/bus/pci/devices/<BDF>/numa_node on the host (via sudo
# since the bats runner may not have read access by default).
host_gpu_numa() {
    sudo cat "/sys/bus/pci/devices/${1}/numa_node"
}

# -----------------------------------------------------------------------------
# Tests
# -----------------------------------------------------------------------------

@test "NUMA: guest topology and host pinning are balanced" {
    # Skip checks must live inside @test (not setup) to avoid bats
    # "Executed 0 instead of expected 1 tests" warnings.
    local skip_reason
    skip_reason=$(numa_skip_reason)
    [[ -z "${skip_reason}" ]] || skip "${skip_reason}"

    patch_kata_numa_config

    local host_nodes
    host_nodes=$(host_numa_node_count)

    local guest_logs
    guest_logs=$(deploy_and_get_guest_logs "${NUMA_TEST_VCPUS_LARGE}" "${NUMA_TEST_MEMORY_LARGE}")
    echo "# Guest NUMA output:"
    echo "# ${guest_logs}"

    # --- Guest topology matches host ---
    local online guest_count
    online=$(guest_field "${guest_logs}" numa_online)
    guest_count=$(guest_online_count "${online}")
    echo "# Guest NUMA online: ${online} -> ${guest_count} node(s); host has ${host_nodes}"
    [[ "${guest_count}" -eq "${host_nodes}" ]] \
        || die "guest NUMA node count (${guest_count}) != host (${host_nodes})"

    # --- Guest vCPU balance ---
    mapfile -t guest_cpus < <(guest_per_node_values "${guest_logs}" _cpus)
    echo "# Guest vCPUs per node: ${guest_cpus[*]}"
    [[ ${#guest_cpus[@]} -ge 2 ]] \
        || die "expected >= 2 guest NUMA buckets, got ${#guest_cpus[@]}"
    local diff
    diff=$(minmax_diff "${guest_cpus[@]}")
    echo "# Guest vCPU balance diff: ${diff}"
    [[ "${diff}" -le 1 ]] || die "guest vCPU imbalance: ${guest_cpus[*]}"

    # --- Guest memory presence per node ---
    mapfile -t guest_mem < <(guest_per_node_values "${guest_logs}" _mem_kb)
    echo "# Guest memory per node (kB): ${guest_mem[*]}"
    [[ ${#guest_mem[@]} -ge 2 ]] || die "expected >= 2 guest memory nodes"

    # --- Host-side vCPU pinning balance ---
    local qemu_pid host_output
    qemu_pid=$(get_qemu_pid_for_pod "${POD_NAME_NUMA}")
    echo "# QEMU PID: ${qemu_pid}"
    host_output=$(wait_for_host_pinning "${qemu_pid}" "${NUMA_TEST_VCPUS_LARGE}")
    echo "# Host pinning per NUMA node: ${host_output}"

    mapfile -t host_counts < <(echo "${host_output}" | grep -oP '^node[0-9]+:\s*\K\d+')
    [[ ${#host_counts[@]} -ge 2 ]] \
        || die "expected >= 2 host NUMA buckets, got ${#host_counts[@]}: ${host_output}"
    diff=$(minmax_diff "${host_counts[@]}")
    echo "# Host pinning diff: ${diff}"
    [[ "${diff}" -le 1 ]] || die "host pinning imbalance: ${host_output}"

    assert_no_shared_host_cpus "${host_output}"
}

@test "NUMA: small workload right-sizes to a single guest NUMA node" {
    # When the sandbox CPU + memory budget fits comfortably on a single
    # host NUMA node and no explicit numa_mapping is provided, the
    # runtime should collapse the auto-derived multi-node topology to a
    # single node to preserve memory locality.  This test exercises
    # selectNUMANodes()'s right-sizing path on a multi-NUMA host:
    #   1. The guest sees exactly one NUMA node with all vCPUs in it.
    #   2. The host-side QEMU vCPU threads are all pinned to that one
    #      host NUMA node (delivered by checkVCPUsPinningNUMA, which
    #      handles single-node sandboxes too).
    local skip_reason
    skip_reason=$(numa_skip_reason)
    [[ -z "${skip_reason}" ]] || skip "${skip_reason}"

    patch_kata_numa_config

    local guest_logs
    guest_logs=$(deploy_and_get_guest_logs "${NUMA_TEST_VCPUS_SMALL}" "${NUMA_TEST_MEMORY_SMALL}")
    echo "# Guest NUMA output:"
    echo "# ${guest_logs}"

    # --- Guest topology collapsed to a single node ---
    local online guest_count
    online=$(guest_field "${guest_logs}" numa_online)
    guest_count=$(guest_online_count "${online}")
    echo "# Guest NUMA online: ${online} -> ${guest_count} node(s)"
    [[ "${guest_count}" -eq 1 ]] \
        || die "right-sized sandbox should expose 1 NUMA node, got ${guest_count}"

    mapfile -t guest_cpus < <(guest_per_node_values "${guest_logs}" _cpus)
    echo "# Guest vCPUs per node: ${guest_cpus[*]}"
    [[ ${#guest_cpus[@]} -eq 1 ]] \
        || die "expected 1 guest NUMA bucket, got ${#guest_cpus[@]}: ${guest_cpus[*]}"
    # The runtime may add a default vCPU on top of the workload request,
    # so the guest can see slightly more than the pod spec asked for.
    [[ "${guest_cpus[0]}" -ge "${NUMA_TEST_VCPUS_SMALL}" ]] \
        || die "expected at least ${NUMA_TEST_VCPUS_SMALL} vCPUs on the single node, got ${guest_cpus[0]}"

    mapfile -t guest_mem < <(guest_per_node_values "${guest_logs}" _mem_kb)
    echo "# Guest memory per node (kB): ${guest_mem[*]}"
    [[ ${#guest_mem[@]} -eq 1 ]] \
        || die "expected 1 guest memory node, got ${#guest_mem[@]}"

    # --- Host-side vCPU pinning collapsed to a single node ---
    local qemu_pid host_output
    qemu_pid=$(get_qemu_pid_for_pod "${POD_NAME_NUMA}")
    echo "# QEMU PID: ${qemu_pid}"
    host_output=$(wait_for_host_pinning "${qemu_pid}" "${NUMA_TEST_VCPUS_SMALL}")
    echo "# Host pinning per NUMA node: ${host_output}"

    mapfile -t host_counts < <(echo "${host_output}" | grep -oP '^node[0-9]+:\s*\K\d+')
    [[ ${#host_counts[@]} -eq 1 ]] \
        || die "right-sized sandbox vCPU threads should land on a single host NUMA node, got ${#host_counts[@]} buckets: ${host_output}"
    [[ "${host_counts[0]}" -ge "${NUMA_TEST_VCPUS_SMALL}" ]] \
        || die "expected at least ${NUMA_TEST_VCPUS_SMALL} vCPU threads placed, got ${host_counts[0]}: ${host_output}"

    assert_no_shared_host_cpus "${host_output}"
    assert_pins_match_exclusive_cpus "${POD_NAME_NUMA}" "${host_output}" "${NUMA_TEST_VCPUS_SMALL}"
}

@test "NUMA: two sandboxes do not claim the same host CPUs" {
    # A sandbox cannot see what the other sandboxes on the node are using, so
    # placing its vCPU threads from a CPU list it does not own puts every
    # sandbox on the same CPUs (kata-containers#13539).  Both sandboxes here
    # are sized alike and land on the same node, which is exactly the case
    # that used to make them pick the same host CPUs.
    #
    # No minimum host NUMA node count: the collision does not need a second
    # host node, and the reported reproduction is a single-node host.
    local skip_reason
    skip_reason=$(numa_unsupported_reason)
    [[ -z "${skip_reason}" ]] || skip "${skip_reason}"

    patch_kata_numa_config

    local guest_logs target_node
    guest_logs=$(deploy_and_get_guest_logs "${NUMA_TEST_VCPUS_PAIR}" "${NUMA_TEST_MEMORY_PAIR}")
    echo "# First sandbox guest NUMA output:"
    echo "# ${guest_logs}"

    target_node=$(kubectl get pod "${POD_NAME_NUMA}" -o jsonpath='{.spec.nodeName}')
    [[ -n "${target_node}" ]] || die "pod ${POD_NAME_NUMA} has no node assigned"
    deploy_second_pod "${NUMA_TEST_VCPUS_PAIR}" "${NUMA_TEST_MEMORY_PAIR}" "${target_node}"
    echo "# Both sandboxes running on node ${target_node}"

    local qemu_pid_a qemu_pid_b output_a output_b
    qemu_pid_a=$(get_qemu_pid_for_pod "${POD_NAME_NUMA}")
    qemu_pid_b=$(get_qemu_pid_for_pod "${POD_NAME_NUMA_B}")
    [[ "${qemu_pid_a}" != "${qemu_pid_b}" ]] \
        || die "both pods resolved to the same QEMU PID ${qemu_pid_a}"

    output_a=$(wait_for_host_pinning "${qemu_pid_a}" "${NUMA_TEST_VCPUS_PAIR}")
    output_b=$(wait_for_host_pinning "${qemu_pid_b}" "${NUMA_TEST_VCPUS_PAIR}")
    echo "# Sandbox A placement: ${output_a}"
    echo "# Sandbox B placement: ${output_b}"

    # Neither sandbox may double-pin on its own ...
    assert_no_shared_host_cpus "${output_a}"
    assert_no_shared_host_cpus "${output_b}"

    # ... nor may the two of them pin onto each other's CPUs.
    local pins_a pins_b shared
    pins_a=$(pinned_cpu_list "${output_a}")
    pins_b=$(pinned_cpu_list "${output_b}")
    echo "# Sandbox A pinned CPUs: ${pins_a:-<none>}"
    echo "# Sandbox B pinned CPUs: ${pins_b:-<none>}"

    shared=$(comm -12 \
        <(echo "${pins_a}" | tr ',' '\n' | grep -v '^$' | sort -u) \
        <(echo "${pins_b}" | tr ',' '\n' | grep -v '^$' | sort -u) \
        | paste -sd,)
    [[ -z "${shared}" ]] \
        || die "both sandboxes pinned vCPU threads to host CPU(s) ${shared}"
}

@test "NUMA: two guest nodes on one host node keep one vCPU per host CPU" {
    # numa_mapping = ["0", "0"] puts both guest NUMA nodes on host node 0, so
    # the two nodes draw their vCPUs from one host CPU list.  The runtime used
    # to walk that list once per guest node and pinned the sandbox's own vCPUs
    # in pairs (0 1 0 1).  Needs one host node only.
    [[ "${KATA_HYPERVISOR}" == qemu-* ]] \
        || skip "explicit numa_mapping test is QEMU-only (got ${KATA_HYPERVISOR})"

    local skip_reason
    skip_reason=$(numa_unsupported_reason)
    [[ -z "${skip_reason}" ]] || skip "${skip_reason}"

    # Patch the active runtime config; teardown() restores it.
    patch_kata_numa_config '["0","0"]'

    local guest_logs
    guest_logs=$(deploy_and_get_guest_logs "${NUMA_TEST_VCPUS_SMALL}" "${NUMA_TEST_MEMORY_SMALL}")
    echo "# Guest NUMA output:"
    echo "# ${guest_logs}"

    # --- Guest sees the two nodes the mapping asked for ---
    local online guest_count
    online=$(guest_field "${guest_logs}" numa_online)
    guest_count=$(guest_online_count "${online}")
    echo "# Guest NUMA online: ${online} -> ${guest_count} node(s)"
    [[ "${guest_count}" -eq 2 ]] \
        || die "numa_mapping=[0,0] should expose 2 guest NUMA nodes, got ${guest_count}"

    # --- Every vCPU thread keeps a host CPU to itself ---
    local qemu_pid host_output
    qemu_pid=$(get_qemu_pid_for_pod "${POD_NAME_NUMA}")
    host_output=$(wait_for_host_pinning "${qemu_pid}" "${NUMA_TEST_VCPUS_SMALL}")
    echo "# Host placement: ${host_output}"

    # Both guest nodes map to host node 0, so all threads bucket there.
    mapfile -t host_counts < <(echo "${host_output}" | grep -oP '^node[0-9]+:\s*\K\d+')
    [[ ${#host_counts[@]} -eq 1 ]] \
        || die "numa_mapping=[0,0] should keep every vCPU on host node 0, got ${#host_counts[@]} buckets: ${host_output}"

    assert_no_shared_host_cpus "${host_output}"
    assert_pins_match_exclusive_cpus "${POD_NAME_NUMA}" "${host_output}" "${NUMA_TEST_VCPUS_SMALL}"
}

@test "NUMA: GPU passthrough with VFIO has correct NUMA placement" {
    local skip_reason
    skip_reason=$(gpu_numa_skip_reason)
    [[ -z "${skip_reason}" ]] || skip "${skip_reason}"

    patch_kata_numa_config

    local host_nodes
    host_nodes=$(host_numa_node_count)

    local gpu_yaml_in="${pod_config_dir}/${POD_NAME_NUMA_GPU}.yaml.in"
    local gpu_yaml="${pod_config_dir}/${POD_NAME_NUMA_GPU}.yaml"

    POD_NAME_NUMA="${POD_NAME_NUMA_GPU}" NUMA_TEST_VCPUS="${NUMA_TEST_VCPUS_GPU}" \
        NUMA_TEST_MEMORY="${NUMA_TEST_MEMORY_GPU}" \
        envsubst < "${gpu_yaml_in}" > "${gpu_yaml}"
    auto_generate_policy "${policy_settings_dir}" "${gpu_yaml}"

    kubectl apply -f "${gpu_yaml}"
    kubectl wait --for=condition=Ready --timeout="${POD_WAIT_TIMEOUT}" pod "${POD_NAME_NUMA_GPU}"
    sleep 2

    local guest_logs
    guest_logs=$(kubectl logs "${POD_NAME_NUMA_GPU}")
    echo "# GPU pod guest NUMA output:"
    echo "# ${guest_logs}"

    # --- Guest NUMA topology matches host ---
    local online guest_count
    online=$(guest_field "${guest_logs}" numa_online)
    guest_count=$(guest_online_count "${online}")
    echo "# Guest NUMA online: ${online} -> ${guest_count} node(s); host has ${host_nodes}"
    [[ "${guest_count}" -eq "${host_nodes}" ]] \
        || die "GPU pod guest NUMA node count (${guest_count}) != host (${host_nodes})"

    # --- Guest vCPU balance ---
    mapfile -t guest_cpus < <(guest_per_node_values "${guest_logs}" _cpus)
    echo "# Guest vCPUs per node: ${guest_cpus[*]}"
    [[ ${#guest_cpus[@]} -ge 2 ]] \
        || die "expected >= 2 guest NUMA buckets, got ${#guest_cpus[@]}"
    local diff
    diff=$(minmax_diff "${guest_cpus[@]}")
    echo "# Guest vCPU balance diff: ${diff}"
    [[ "${diff}" -le 1 ]] || die "GPU pod guest vCPU imbalance: ${guest_cpus[*]}"

    # --- Guest memory presence per node ---
    mapfile -t guest_mem < <(guest_per_node_values "${guest_logs}" _mem_kb)
    echo "# Guest memory per node (kB): ${guest_mem[*]}"
    [[ ${#guest_mem[@]} -ge 2 ]] || die "expected >= 2 guest memory nodes"

    # --- Host-side QEMU lookup (needed for the GPU NUMA assertion) ---
    local qemu_pid qemu_cmd host_bdf host_node
    qemu_pid=$(get_qemu_pid_for_pod "${POD_NAME_NUMA_GPU}")
    echo "# QEMU PID: ${qemu_pid}"

    qemu_cmd=$(get_qemu_cmdline "${qemu_pid}")
    host_bdf=$(extract_vfio_host_bdf "${qemu_cmd}")
    [[ -n "${host_bdf}" ]] || die "no vfio-pci host BDF found in QEMU cmdline"
    host_node=$(host_gpu_numa "${host_bdf}")
    echo "# Host GPU ${host_bdf} on NUMA node ${host_node}"

    # --- Guest GPU NUMA affinity ---
    # With pxb-pcie and default numa_mapping (1:1), the guest GPU's NUMA
    # node must equal the host GPU's NUMA node.
    mapfile -t gpu_numas < <(echo "${guest_logs}" | grep -oP 'gpu_.*_numa:\s*\K-?\d+')
    echo "# Guest GPU NUMA nodes: ${gpu_numas[*]}"
    [[ ${#gpu_numas[@]} -ge 1 ]] \
        || die "no GPU detected in guest sysfs (expected gpu_*_numa: lines)"
    for gn in "${gpu_numas[@]}"; do
        [[ "${gn}" -eq "${host_node}" ]] \
            || die "guest GPU on node ${gn} but host GPU ${host_bdf} is on node ${host_node}"
    done

    # --- Host-side vCPU pinning balance ---
    local host_output
    host_output=$(wait_for_host_pinning "${qemu_pid}" "${NUMA_TEST_VCPUS_GPU}")
    echo "# Host pinning per NUMA node: ${host_output}"

    mapfile -t host_counts < <(echo "${host_output}" | grep -oP '^node[0-9]+:\s*\K\d+')
    [[ ${#host_counts[@]} -ge 2 ]] \
        || die "expected >= 2 host NUMA buckets for GPU pod, got ${#host_counts[@]}: ${host_output}"
    diff=$(minmax_diff "${host_counts[@]}")
    echo "# Host pinning diff: ${diff}"
    [[ "${diff}" -le 1 ]] || die "GPU pod host pinning imbalance: ${host_output}"

    assert_no_shared_host_cpus "${host_output}"

    # --- QEMU command line: pxb-pcie and NUMA binding ---
    echo "# Checking QEMU cmdline for pxb-pcie..."
    [[ "${qemu_cmd}" == *"pxb-pcie"* ]] \
        || die "QEMU command line does not contain 'pxb-pcie' — NUMA PCIe topology not active"

    echo "# Checking QEMU cmdline for NUMA memory binding..."
    [[ "${qemu_cmd}" == *"policy=bind"* ]] \
        || die "QEMU command line does not contain 'policy=bind' — NUMA memory binding not active"
}

@test "NUMA: small GPU workload right-sizes to the GPU's host NUMA node" {
    # When a GPU is attached and the sandbox CPU + memory budget fits on
    # a single host NUMA node, the runtime's right-sizing path
    # (selectNUMANodes with VFIO awareness) should collapse the topology
    # to the GPU's host NUMA node — not just any fitting node — so that
    # GPU and memory access stay NUMA-local.
    local skip_reason
    skip_reason=$(gpu_numa_skip_reason)
    [[ -z "${skip_reason}" ]] || skip "${skip_reason}"

    patch_kata_numa_config

    local gpu_yaml_in="${pod_config_dir}/${POD_NAME_NUMA_GPU}.yaml.in"
    local gpu_yaml="${pod_config_dir}/${POD_NAME_NUMA_GPU}.yaml"

    POD_NAME_NUMA="${POD_NAME_NUMA_GPU}" NUMA_TEST_VCPUS="${NUMA_TEST_VCPUS_GPU_SMALL}" \
        NUMA_TEST_MEMORY="${NUMA_TEST_MEMORY_GPU_SMALL}" \
        envsubst < "${gpu_yaml_in}" > "${gpu_yaml}"
    auto_generate_policy "${policy_settings_dir}" "${gpu_yaml}"

    kubectl apply -f "${gpu_yaml}"
    kubectl wait --for=condition=Ready --timeout="${POD_WAIT_TIMEOUT}" pod "${POD_NAME_NUMA_GPU}"
    sleep 2

    local guest_logs
    guest_logs=$(kubectl logs "${POD_NAME_NUMA_GPU}")
    echo "# Small GPU pod guest NUMA output:"
    echo "# ${guest_logs}"

    # --- Host-side QEMU lookup ---
    local qemu_pid qemu_cmd host_bdf host_node
    qemu_pid=$(get_qemu_pid_for_pod "${POD_NAME_NUMA_GPU}")

    qemu_cmd=$(get_qemu_cmdline "${qemu_pid}")
    host_bdf=$(extract_vfio_host_bdf "${qemu_cmd}")
    [[ -n "${host_bdf}" ]] || die "no vfio-pci host BDF found in QEMU cmdline"
    host_node=$(host_gpu_numa "${host_bdf}")
    echo "# Host GPU ${host_bdf} on NUMA node ${host_node}"

    # --- Guest collapsed to a single NUMA node ---
    local online guest_count
    online=$(guest_field "${guest_logs}" numa_online)
    guest_count=$(guest_online_count "${online}")
    echo "# Guest NUMA online: ${online} -> ${guest_count} node(s)"
    [[ "${guest_count}" -eq 1 ]] \
        || die "right-sized GPU sandbox should expose 1 NUMA node, got ${guest_count}"

    # --- Guest GPU sees the (single) node ---
    mapfile -t gpu_numas < <(echo "${guest_logs}" | grep -oP 'gpu_.*_numa:\s*\K-?\d+')
    echo "# Guest GPU NUMA nodes: ${gpu_numas[*]}"
    [[ ${#gpu_numas[@]} -ge 1 ]] \
        || die "no GPU detected in guest sysfs (expected gpu_*_numa: lines)"
    # In a single-node guest, the GPU is on node 0.
    for gn in "${gpu_numas[@]}"; do
        [[ "${gn}" -eq 0 ]] \
            || die "guest GPU on node ${gn} but right-sized sandbox has only node 0"
    done

    # --- QEMU memory backend bound to the GPU's host NUMA node ---
    # The right-sizing path should pick the GPU's host node, not just
    # any node that fits. With pxb-pcie + right-sizing, the single
    # memory-backend-ram for the sandbox must have host-nodes=${host_node}.
    echo "# Checking QEMU cmdline for memory binding on host node ${host_node}..."
    [[ "${qemu_cmd}" == *"host-nodes=${host_node}"* ]] \
        || die "right-sized GPU sandbox memory not bound to GPU's host NUMA node ${host_node}: cmdline=${qemu_cmd}"

    # --- Host-side vCPU pinning collapsed to the GPU's host node ---
    local host_output
    host_output=$(wait_for_host_pinning "${qemu_pid}" "${NUMA_TEST_VCPUS_GPU_SMALL}")
    echo "# Host pinning per NUMA node: ${host_output}"

    mapfile -t host_counts < <(echo "${host_output}" | grep -oP '^node[0-9]+:\s*\K\d+')
    [[ ${#host_counts[@]} -eq 1 ]] \
        || die "right-sized GPU sandbox vCPU threads should land on a single host NUMA node, got ${#host_counts[@]} buckets: ${host_output}"

    local pinned_node
    pinned_node=$(echo "${host_output}" | grep -oP '^node\K[0-9]+' | head -1)
    [[ "${pinned_node}" -eq "${host_node}" ]] \
        || die "right-sized GPU sandbox vCPUs pinned to node ${pinned_node} but GPU is on host node ${host_node}"

    assert_no_shared_host_cpus "${host_output}"
}

@test "NUMA: explicit numa_mapping in TOML pins the sandbox to the chosen host node" {
    # When the user sets numa_mapping = ["1"] in the runtime TOML, the
    # right-sizing path must be skipped (maybeRightSizeAutoNUMA bails out
    # for non-empty NUMAMapping) and buildNUMATopology must propagate the
    # binding verbatim, regardless of how small the workload is.
    #
    # Verifies end-to-end that:
    #   - guest sees exactly 1 NUMA node;
    #   - the QEMU memory backend is bound to host node 1 (not 0);
    #   - host-side vCPU threads land on host node 1.
    #
    # QEMU-only: this test asserts on the QEMU command line (host-nodes=,
    # policy=bind) and on the kata-runtime (Go) NUMA logic.  runtime-rs
    # does not yet implement NUMA, so even if numa_skip_reason were
    # widened later we'd still want to gate this case explicitly.
    [[ "${KATA_HYPERVISOR}" == qemu-* ]] \
        || skip "explicit numa_mapping test is QEMU-only (got ${KATA_HYPERVISOR})"

    local skip_reason
    skip_reason=$(numa_skip_reason)
    [[ -z "${skip_reason}" ]] || skip "${skip_reason}"

    # Need at least 2 host nodes so "host node 1" is a non-trivial pick.
    local host_nodes
    host_nodes=$(host_numa_node_count)
    [[ "${host_nodes}" -ge 2 ]] || skip "explicit-mapping test needs >=2 host NUMA nodes"

    # Patch the active runtime config; teardown() restores it.
    patch_kata_numa_config '["1"]'

    local guest_logs
    guest_logs=$(deploy_and_get_guest_logs "${NUMA_TEST_VCPUS_SMALL}" "${NUMA_TEST_MEMORY_SMALL}")
    echo "# Guest NUMA output:"
    echo "# ${guest_logs}"

    # --- Guest: explicit mapping always yields exactly one node ---
    local online guest_count
    online=$(guest_field "${guest_logs}" numa_online)
    guest_count=$(guest_online_count "${online}")
    echo "# Guest NUMA online: ${online} -> ${guest_count} node(s)"
    [[ "${guest_count}" -eq 1 ]] \
        || die "explicit numa_mapping=[1] should expose 1 guest NUMA node, got ${guest_count}"

    # --- QEMU memory backend bound to host node 1 ---
    local qemu_pid qemu_cmd
    qemu_pid=$(get_qemu_pid_for_pod "${POD_NAME_NUMA}")
    qemu_cmd=$(get_qemu_cmdline "${qemu_pid}")
    echo "# Checking QEMU cmdline for memory binding on host node 1..."
    [[ "${qemu_cmd}" == *"host-nodes=1"* ]] \
        || die "explicit numa_mapping=[1] did not pin QEMU memory to host node 1: cmdline=${qemu_cmd}"
    [[ "${qemu_cmd}" == *"policy=bind"* ]] \
        || die "explicit numa_mapping=[1] missing policy=bind in QEMU cmdline: cmdline=${qemu_cmd}"

    # --- Host-side vCPU pinning lands on host node 1 ---
    local host_output
    host_output=$(wait_for_host_pinning "${qemu_pid}" "${NUMA_TEST_VCPUS_SMALL}")
    echo "# Host pinning per NUMA node: ${host_output}"

    mapfile -t host_counts < <(echo "${host_output}" | grep -oP '^node[0-9]+:\s*\K\d+')
    [[ ${#host_counts[@]} -eq 1 ]] \
        || die "explicit numa_mapping=[1] should pin vCPUs to a single host NUMA node, got ${#host_counts[@]} buckets: ${host_output}"

    local pinned_node
    pinned_node=$(echo "${host_output}" | grep -oP '^node\K[0-9]+' | head -1)
    [[ "${pinned_node}" -eq 1 ]] \
        || die "explicit numa_mapping=[1] pinned vCPUs to node ${pinned_node}, expected 1"

    assert_no_shared_host_cpus "${host_output}"
}

teardown() {
    echo "=== NUMA test pod describe ==="
    kubectl describe pod "${POD_NAME_NUMA}" || true
    kubectl describe pod "${POD_NAME_NUMA_B}" 2>/dev/null || true
    kubectl describe pod "${POD_NAME_NUMA_GPU}" 2>/dev/null || true

    echo "=== NUMA test pod logs ==="
    kubectl logs "${POD_NAME_NUMA}" || true
    kubectl logs "${POD_NAME_NUMA_B}" 2>/dev/null || true
    kubectl logs "${POD_NAME_NUMA_GPU}" 2>/dev/null || true

    # Always restore the Kata config (no-op if no patch was applied).
    restore_kata_numa_config || true

    delete_tmp_policy_settings_dir "${policy_settings_dir}"

    [ -f "${pod_yaml}" ] && kubectl delete -f "${pod_yaml}" --ignore-not-found=true
    [ -f "${pod_yaml_b}" ] && kubectl delete -f "${pod_yaml_b}" --ignore-not-found=true
    local gpu_yaml="${pod_config_dir}/${POD_NAME_NUMA_GPU}.yaml"
    [ -f "${gpu_yaml}" ] && kubectl delete -f "${gpu_yaml}" --ignore-not-found=true

    print_node_journal_since_test_start "${node}" "${node_start_time:-}" "${BATS_TEST_COMPLETED:-}"
}
