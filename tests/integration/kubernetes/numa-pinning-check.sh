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
# Output: one line per NUMA node with the count of pinned vCPU threads.
#   node0: 32
#   node1: 32
#
# A vCPU thread is counted only when taskset reports it pinned to a single
# CPU (bare number, no ranges or commas).  Threads with broad affinity
# masks are silently skipped — the caller is expected to retry until the
# runtime has finished per-vCPU pinning.

set -o pipefail

QEMU_PID="${1:?Usage: $0 <qemu_pid>}"

if [[ ! -d "/proc/${QEMU_PID}/task" ]]; then
    echo "ERROR: /proc/${QEMU_PID}/task not found" >&2
    exit 1
fi

for tid in "/proc/${QEMU_PID}/task/"*; do
    tid="${tid##*/}"
    list=$(taskset -pc "${tid}" 2>/dev/null | sed 's/.*: //')
    if [[ "${list}" =~ ^[0-9]+$ ]]; then
        # Map the CPU to its NUMA node via the sysfs topology symlink
        for node_link in "/sys/devices/system/cpu/cpu${list}/node"*; do
            if [[ -d "${node_link}" ]]; then
                numa_node="${node_link##*node}"
                echo "node${numa_node}"
                break
            fi
        done
    fi
done | sort | uniq -c | awk '{print $2 ": " $1}'
