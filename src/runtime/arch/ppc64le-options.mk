# Copyright (c) 2018 IBM
#
# SPDX-License-Identifier: Apache-2.0
#

# Power ppc64le settings

MACHINETYPE := pseries
KERNELPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS := "cap-cfpc=broken,cap-sbbc=broken,cap-ibs=broken,cap-large-decr=off,cap-ccf-assist=off"
CPUFEATURES :=
KERNELTYPE := uncompressed #This architecture must use an uncompressed kernel.
QEMUCMD := qemu-system-ppc64
