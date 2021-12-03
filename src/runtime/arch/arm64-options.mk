# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# ARM 64 settings

KERNELPARAMS :=
MACHINEACCELERATORS :=
CPUFEATURES := pmu=off

ifeq ($(HOST_OS),Linux)
       MACHINETYPE := virt

       QEMUCMD := qemu-system-aarch64

       # Firecracker binary name
       FCCMD := firecracker
       # Firecracker's jailer binary name
       FCJAILERCMD := jailer

       # cloud-hypervisor binary name
       CLHCMD := cloud-hypervisor
endif

# Virtualization.framework
ifeq ($(HOST_OS),Darwin)
       VFW := true
endif
