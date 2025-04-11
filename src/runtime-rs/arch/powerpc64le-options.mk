# Copyright (c) 2019-2022 Alibaba Cloud
# Copyright (c) 2019-2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

MACHINETYPE := pseries
KERNELPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS := "cap-cfpc=broken,cap-sbbc=broken,cap-ibs=broken,cap-large-decr=off,cap-ccf-assist=off"
CPUFEATURES := pmu=off

QEMUCMD := qemu-system-ppc64

# dragonball binary name
DBCMD := dragonball
