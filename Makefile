#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

DISTRO ?= centos
DISTRO_ROOTFS := "$(PWD)/$(DISTRO)_rootfs"
IMG_SIZE=500
AGENT_INIT ?= no

all: rootfs image initrd
rootfs:
	@echo Creating rootfs based on "$(DISTRO)"
	"$(MK_DIR)/rootfs-builder/rootfs.sh" -r "$(DISTRO_ROOTFS)" "$(DISTRO)"

image: rootfs image-only

image-only:
	@echo Creating image based on "$(DISTRO_ROOTFS)"
	"$(MK_DIR)/image-builder/image_builder.sh" -s "$(IMG_SIZE)" "$(DISTRO_ROOTFS)"

initrd: rootfs initrd-only

initrd-only:
	@echo Creating initrd image based on "$(DISTRO_ROOTFS)"
	"$(MK_DIR)/initrd-builder/initrd_builder.sh" "$(DISTRO_ROOTFS)"
