# Copyright (c) 2019-2022 Alibaba Cloud
# Copyright (c) 2019-2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

# ARM 64 settings

MACHINETYPE := virt
KERNELPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS := usb=off,gic-version=host
CPUFEATURES := pmu=off

QEMUCMD := qemu-system-aarch64
QEMUNOSHAREDFSCMD := qemu-system-aarch64-no-shared-fs
# NVIDIA's QEMU fork; the GPU runtime class boots it instead of the trimmed build.
QEMUNVIDIAGPUCMD := qemu-system-aarch64-cca-experimental
QEMUFW := AAVMF_CODE.fd

# dragonball binary name
DBCMD := dragonball
FCCMD := firecracker
FCJAILERCMD := jailer
