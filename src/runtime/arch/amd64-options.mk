# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Intel x86-64 settings

MACHINETYPE := pc
KERNELPARAMS :=
MACHINEACCELERATORS :=
CPUFEATURES := pmu=off

QEMUCMD := qemu-system-x86_64

# Firecracker binary name
FCCMD := firecracker
# Firecracker's jailer binary name
FCJAILERCMD := jailer

#ACRN binary name
ACRNCMD := acrn-dm
ACRNCTLCMD := acrnctl

# cloud-hypervisor binary name
CLHCMD := cloud-hypervisor
