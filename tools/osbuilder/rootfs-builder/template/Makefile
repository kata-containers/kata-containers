# Copyright (c) 2017 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#
MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
## Default destdir is one level up where is rootfs.sh script
DESTDIR ?= "$(realpath $(MK_DIR)/../)/$(ROOTFS_BASE_NAME)"
all:
ifndef ROOTFS_BASE_NAME
	$(error ROOTFS_BASE_NAME is not set, use $ make ROOTFS_BASE_NAME=new_supported_os)
endif
	mkdir -p $(DESTDIR)
	cp "$(MK_DIR)/rootfs_lib_template.sh" "$(DESTDIR)/rootfs_lib.sh"
	cp "$(MK_DIR)/config_template.sh" "$(DESTDIR)/config.sh"
	sed \
		-e "s|@distro@|$(ROOTFS_BASE_NAME)|g" \
		"$(MK_DIR)/Dockerfile.template" > $(DESTDIR)/Dockerfile.in


