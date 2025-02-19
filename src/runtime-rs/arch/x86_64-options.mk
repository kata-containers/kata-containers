# Copyright (c) 2019-2022 Alibaba Cloud
# Copyright (c) 2019-2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

MACHINETYPE := q35
KERNELPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
KERNELTDXPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS :=
CPUFEATURES := pmu=off

QEMUCMD := qemu-system-x86_64

# dragonball binary name
DBCMD := dragonball

# cloud-hypervisor binary name
CLHCMD := cloud-hypervisor

# firecracker binary (vmm and jailer)
FCCMD := firecracker
FCJAILERCMD := jailer

REMOTE := remote
