# Copyright (c) 2018 IBM
#
# SPDX-License-Identifier: Apache-2.0
#

# s390x settings

MACHINETYPE := s390-ccw-virtio
KERNELPARAMS := cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
MACHINEACCELERATORS :=
CPUFEATURES :=

QEMUCMD := qemu-system-s390x

# See https://github.com/kata-containers/osbuilder/issues/217
NEEDS_CC_SETTING = $(shell grep -E "\<(fedora|suse)\>" /etc/os-release 2> /dev/null)
ifneq (,$(NEEDS_CC_SETTING))
	CC := gcc
	export CC
endif
