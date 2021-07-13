# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

MK_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MK_DIR := $(dir $(MK_PATH))

# Verbose build
V ?=
ifneq ($(V),)
SILENT_BUILD_FLAG =
else
SILENT_BUILD_FLAG = -s
endif

define BUILD
	$(MK_DIR)/kata-deploy-binaries-in-docker.sh $(SILENT_BUILD_FLAG) --build=$1
endef

kata-tarball: | all-parallel merge-builds

all-parallel:
	make -f $(MK_PATH) all -j$$(( $$(nproc) - 1  ))

all: cloud-hypervisor firecracker kernel qemu rootfs-image rootfs-initrd shim-v2

cloud-hypervisor:
	$(call BUILD,$@)

firecracker:
	$(call BUILD,$@)

kernel:
	$(call BUILD,$@)

qemu:
	$(call BUILD,$@)

rootfs-image:
	$(call BUILD,$@)

rootfs-initrd:
	$(call BUILD,$@)

shim-v2:
	$(call BUILD,$@)

merge-builds:
	$(MK_DIR)/kata-deploy-merge-builds.sh build

install:
	tar -xvf ./kata-static.tar.xz -C /
