# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Intel x86-64 settings

MACHINETYPE := q35
KERNELPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
KERNELTDXPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS :=
CPUFEATURES := pmu=off

QEMUCMD := qemu-system-x86_64
QEMUTDXCMD := qemu-system-x86_64-tdx-experimental
QEMUSNPCMD := qemu-system-x86_64-snp-experimental
TDXCPUFEATURES := -vmx-rdseed-exit,pmu=off

# Firecracker binary name
FCCMD := firecracker
# Firecracker's jailer binary name
FCJAILERCMD := jailer

# cloud-hypervisor binary name
CLHCMD := cloud-hypervisor

DEFSTATICRESOURCEMGMT_CLH := false

# stratovirt binary name
STRATOVIRTCMD := stratovirt
