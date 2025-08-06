# Copyright (c) 2019-2022 Alibaba Cloud
# Copyright (c) 2019-2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

MACHINETYPE := s390-ccw-virtio
KERNELPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS :=
CPUFEATURES :=

QEMUCMD := qemu-system-s390x
