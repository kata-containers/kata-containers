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

VERSION_FILE := ./VERSION
VERSION := $(shell grep -v ^\# $(VERSION_FILE))
COMMIT_NO := $(shell git rev-parse HEAD 2> /dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no),${COMMIT_NO}-dirty,${COMMIT_NO})
VERSION_COMMIT := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))

all: rootfs image initrd
rootfs:
	@echo Creating rootfs based on "$(DISTRO)"
	"$(MK_DIR)/rootfs-builder/rootfs.sh" -o $(VERSION_COMMIT) -r "$(DISTRO_ROOTFS)" "$(DISTRO)"

image: rootfs image-only

image-only:
	@echo Creating image based on "$(DISTRO_ROOTFS)"
	"$(MK_DIR)/image-builder/image_builder.sh" -s "$(IMG_SIZE)" "$(DISTRO_ROOTFS)"

initrd: rootfs initrd-only

initrd-only:
	@echo Creating initrd image based on "$(DISTRO_ROOTFS)"
	"$(MK_DIR)/initrd-builder/initrd_builder.sh" "$(DISTRO_ROOTFS)"

test:
	"$(MK_DIR)/tests/test_images.sh" "$(DISTRO)"

test-image-only:
	"$(MK_DIR)/tests/test_images.sh" --test-images-only "$(DISTRO)"

test-initrd-only:
	"$(MK_DIR)/tests/test_images.sh" --test-initrds-only "$(DISTRO)"
