# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# ARM 64 settings

MACHINETYPE := virt
KERNELPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS :=
CPUFEATURES := pmu=off

QEMUCMD := qemu-system-aarch64
QEMUCCAEXPERIMENTALCMD := qemu-system-aarch64-cca-experimental
QEMUFW := AAVMF_CODE.fd
QEMUFWVOL := AAVMF_VARS.fd

# Firecracker binary name
FCCMD := firecracker
# Firecracker's jailer binary name
FCJAILERCMD := jailer

# cloud-hypervisor binary name
CLHCMD := cloud-hypervisor

DEFSTATICRESOURCEMGMT_CLH := true

# stratovirt binary name
STRATOVIRTCMD := stratovirt
