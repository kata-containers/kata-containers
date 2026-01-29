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
QEMUFW := AAVMF_CODE.fd
QEMUFWVOL := AAVMF_VARS.fd

# dragonball binary name
DBCMD := dragonball
FCCMD := firecracker
FCJAILERCMD := jailer
