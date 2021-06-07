# Copyright (c) 2018 IBM
#
# SPDX-License-Identifier: Apache-2.0
#

# s390x settings

MACHINETYPE := s390-ccw-virtio
KERNELPARAMS :=
MACHINEACCELERATORS :=
CPUFEATURES :=

QEMUCMD := qemu-system-s390x

# See https://github.com/kata-containers/osbuilder/issues/217
FEDORA_LIKE = $(shell grep -E "\<fedora\>" /etc/os-release 2> /dev/null)
ifneq (,$(FEDORA_LIKE))
	CC := gcc
	export CC
endif
