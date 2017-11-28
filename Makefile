#
# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

DISTRO ?= centos
DISTRO_ROOTFS := "$(PWD)/$(DISTRO)_rootfs"
IMG_SIZE=500

image:
	@echo Creating rootfs based on "$(DISTRO)"
	"$(MK_DIR)/rootfs-builder/rootfs.sh" -r "$(DISTRO_ROOTFS)" "$(DISTRO)"
	@echo Creating image based on "$(DISTRO_ROOTFS)"
	AGENT_BIN="$(AGENT_BIN)" "$(MK_DIR)/image-builder/image_builder.sh" -s "$(IMG_SIZE)" "$(DISTRO_ROOTFS)"
