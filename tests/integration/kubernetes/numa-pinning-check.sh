#!/bin/bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# WARNING: This script runs directly on the host, NOT inside a container.
# It requires privileged access to /proc and /sys to inspect QEMU vCPU
# thread affinities and map them to host NUMA nodes.
#
# Usage: numa-pinning-check.sh <qemu_pid>
#
# Output: one line per NUMA node with the count of vCPU threads confined to
# it, then the two totals that tell whether any host CPU carries more than
# one vCPU thread, then the host CPUs this QEMU claimed for a single thread:
#   node0: 32
#   node1: 32
#   single_cpu_threads: 64
#   distinct_single_cpus: 64
#   pinned_cpus: 0,1,2,3
#
# Only KVM vCPU threads ("CPU 3/KVM") are looked at, and one counts towards a
# node when every CPU it may run on belongs to that node.  That covers both
# ways the runtime places vCPUs: one host CPU per vCPU thread, and the NUMA
# node affinity mask it falls back to when it cannot give each vCPU a host
# CPU of its own.  Threads still allowed to run across nodes are skipped —
# the caller is expected to retry until the runtime has placed the vCPUs.
#
# single_cpu_threads exceeds distinct_single_cpus when several vCPU threads
# are pinned to the same host CPU, which makes them time-share it while other
# CPUs sit idle.  pinned_cpus lets a caller compare two sandboxes and catch
# the same overlap happening across them.

set -o pipefail

QEMU_PID="${1:?Usage: $0 <qemu_pid>}"

if [[ ! -d "/proc/${QEMU_PID}/task" ]]; then
    echo "ERROR: /proc/${QEMU_PID}/task not found" >&2
    exit 1
fi

# expand_cpu_list <list> emits one CPU per line for a cpuset-style list such
# as "0-3,8".
expand_cpu_list() {
    local item cpu
    local IFS=,
    for item in ${1}; do
        if [[ "${item}" =~ ^([0-9]+)-([0-9]+)$ ]]; then
            for ((cpu = BASH_REMATCH[1]; cpu <= BASH_REMATCH[2]; cpu++)); do
                echo "${cpu}"
            done
        elif [[ "${item}" =~ ^[0-9]+$ ]]; then
            echo "${item}"
        fi
    done
}

# cpu_node <cpu> echoes the NUMA node a host CPU belongs to.
cpu_node() {
    local node_link
    for node_link in "/sys/devices/system/cpu/cpu${1}/node"*; do
        if [[ -d "${node_link}" ]]; then
            echo "${node_link##*node}"
            return 0
        fi
    done
    return 1
}

declare -A node_threads=()
declare -A pinned_cpus=()
single_cpu_threads=0

for task in "/proc/${QEMU_PID}/task/"*; do
    [[ -r "${task}/comm" && -r "${task}/status" ]] || continue
    [[ "$(<"${task}/comm")" =~ ^CPU\ [0-9]+/KVM$ ]] || continue

    list=$(grep -oP '^Cpus_allowed_list:\s*\K.*' "${task}/status")
    mapfile -t cpus < <(expand_cpu_list "${list}")
    (( ${#cpus[@]} > 0 )) || continue

    node=""
    for cpu in "${cpus[@]}"; do
        this_node=$(cpu_node "${cpu}") || { node=""; break; }
        if [[ -z "${node}" ]]; then
            node="${this_node}"
        elif [[ "${this_node}" != "${node}" ]]; then
            node=""
            break
        fi
    done
    # Still free to run on more than one node: not placed (yet).
    [[ -n "${node}" ]] || continue

    node_threads["${node}"]=$(( ${node_threads["${node}"]:-0} + 1 ))
    if (( ${#cpus[@]} == 1 )); then
        single_cpu_threads=$(( single_cpu_threads + 1 ))
        pinned_cpus["${cpus[0]}"]=1
    fi
done

if (( ${#node_threads[@]} > 0 )); then
    while read -r node; do
        echo "node${node}: ${node_threads[${node}]}"
    done < <(printf '%s\n' "${!node_threads[@]}" | sort -n)
fi

echo "single_cpu_threads: ${single_cpu_threads}"
echo "distinct_single_cpus: ${#pinned_cpus[@]}"

pinned_list=""
if (( ${#pinned_cpus[@]} > 0 )); then
    pinned_list=$(printf '%s\n' "${!pinned_cpus[@]}" | sort -n | paste -sd,)
fi
echo "pinned_cpus: ${pinned_list}"
