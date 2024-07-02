# Copyright (c) 2019-2022 Alibaba Cloud
# Copyright (c) 2019-2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

MACHINETYPE := q35
KERNELPARAMS :=
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
