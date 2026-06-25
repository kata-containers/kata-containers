#!/bin/sh
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Reads guest NUMA topology from sysfs and prints structured output.
# Designed to run inside a kata VM as the container entrypoint.
#
# Output format (one key: value per line):
#   numa_online: 0-1
#   node0_cpus: 32
#   node1_cpus: 32
#   node0_mem_kb: 37078332
#   node1_mem_kb: 37125524
#   gpu_0000:41:00.0_numa: 1       (only if GPUs are present)

set -e

# Print results to stdout (readable via "kubectl logs"), then sleep to
# keep the pod alive so the host-side pinning check can inspect the
# QEMU process.  The bats test deletes the pod when done.

# NUMA nodes online (e.g. "0-1" or "0")
online=$(cat /sys/devices/system/node/online)
echo "numa_online: ${online}"

# Per-node vCPU count
for cpulist in /sys/devices/system/node/node*/cpulist; do
    node_name=$(basename "$(dirname "${cpulist}")")
    cpus=$(cat "${cpulist}")
    count=0
    # Parse comma-separated ranges like "0-31,64-95"
    IFS=","
    for range in ${cpus}; do
        case "${range}" in
            *-*)
                lo=${range%-*}
                hi=${range#*-}
                count=$((count + hi - lo + 1))
                ;;
            *)
                count=$((count + 1))
                ;;
        esac
    done
    unset IFS
    echo "${node_name}_cpus: ${count}"
done

# Per-node memory
for meminfo in /sys/devices/system/node/node*/meminfo; do
    node_name=$(basename "$(dirname "${meminfo}")")
    mem_kb=$(awk '/MemTotal/ {print $4}' "${meminfo}")
    echo "${node_name}_mem_kb: ${mem_kb}"
done

# GPU NUMA affinity (if any GPUs are present via VFIO passthrough).
# PCI class 0x030200 = 3D controller (NVIDIA data center GPUs: A100, H100, etc.)
for numa_file in /sys/bus/pci/devices/*/numa_node; do
    dev_dir=$(dirname "${numa_file}")
    class=$(cat "${dev_dir}/class" 2>/dev/null) || continue
    case "${class}" in
        0x030200)
            bdf=$(basename "${dev_dir}")
            node=$(cat "${numa_file}")
            echo "gpu_${bdf}_numa: ${node}"
            ;;
    esac
done

# Keep the pod alive for host-side pinning verification.
exec sleep infinity
