#!/bin/bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# WARNING: This script runs directly on the host, NOT inside a container.
# It requires privileged access to /proc and taskset to inspect QEMU vCPU
# thread CPU affinities.
#
# Usage: vcpu-pinning-check.sh <qemu_pid>
#
# For every "CPU N/KVM" thread found in the QEMU process, print one line:
#   pinned <vcpu_idx> <host_cpu>     — thread is pinned to exactly one host CPU
#   floating <vcpu_idx>              — thread affinity spans multiple CPUs
#
# Lines are emitted in ascending vCPU-index order.  A thread is counted as
# pinned only when taskset reports a bare integer (single CPU).

set -o pipefail

QEMU_PID="${1:?Usage: $0 <qemu_pid>}"

if [[ ! -d "/proc/${QEMU_PID}/task" ]]; then
    echo "ERROR: /proc/${QEMU_PID}/task not found" >&2
    exit 1
fi

for tid_dir in "/proc/${QEMU_PID}/task/"*; do
    tid="${tid_dir##*/}"
    comm=$(cat "${tid_dir}/comm" 2>/dev/null || true)

    # Only inspect vCPU KVM threads (names like "CPU 0/KVM", "CPU 1/KVM", …).
    [[ "${comm}" =~ ^CPU\ ([0-9]+)/KVM$ ]] || continue
    vcpu_idx="${BASH_REMATCH[1]}"

    list=$(taskset -pc "${tid}" 2>/dev/null | sed 's/.*: //')
    if [[ "${list}" =~ ^[0-9]+$ ]]; then
        # Pinned to a single host CPU.
        printf "pinned %d %d\n" "${vcpu_idx}" "${list}"
    else
        # Floating across multiple CPUs (not 1:1 pinned).
        printf "floating %d\n" "${vcpu_idx}"
    fi
done | sort -k2 -n
