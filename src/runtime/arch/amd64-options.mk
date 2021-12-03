# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

HOST_OS := $(shell uname)

# Intel x86-64 settings

KERNELPARAMS :=
MACHINEACCELERATORS :=
CPUFEATURES := pmu=off

ifeq ($(HOST_OS),Linux)
       MACHINETYPE := q35

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
endif

# Virtualization.framework
ifeq ($(HOST_OS),Darwin)
       VFW := true
endif
