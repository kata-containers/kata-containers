# Copyright (c) 2026 Loongson Technology Corporation Limited.
#
# SPDX-License-Identifier: Apache-2.0
#

# LoongArch 64 settings

MACHINETYPE := virt
KERNELPARAMS :=  cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS :=
CPUFEATURES :=

QEMUCMD := qemu-system-loongarch64
