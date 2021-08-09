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
	${MAKE} -f $(MK_PATH) all -j$$(( $$(nproc) - 1  )) NO_TTY="true"

all: cloud-hypervisor-tarball \
	firecracker-tarball \
	kernel-tarball \
	qemu-tarball \
	rootfs-image-tarball \
	rootfs-initrd-tarball \
	shim-v2-tarball

%-tarball-build:
	$(call BUILD,$*)

cloud-hypervisor-tarball:
	${MAKE} $@-build

firecracker-tarball:
	${MAKE} $@-build

kernel-tarball:
	${MAKE} $@-build

qemu-tarball:
	${MAKE} $@-build

rootfs-image-tarball:
	${MAKE} $@-build

rootfs-initrd-tarball:
	${MAKE} $@-build

shim-v2-tarball:
	${MAKE} $@-build

merge-builds:
	$(MK_DIR)/kata-deploy-merge-builds.sh build

install-tarball:
	tar -xvf ./kata-static.tar.xz -C /
