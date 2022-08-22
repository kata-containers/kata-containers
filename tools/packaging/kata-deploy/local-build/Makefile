# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

MK_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MK_DIR := $(dir $(MK_PATH))

# Verbose build
V := 1

define BUILD
	$(MK_DIR)/kata-deploy-binaries-in-docker.sh $(if $(V),,-s) --build=$1
endef

kata-tarball: | all-parallel merge-builds

$(MK_DIR)/dockerbuild/install_yq.sh:
	$(MK_DIR)/kata-deploy-copy-yq-installer.sh

all-parallel: $(MK_DIR)/dockerbuild/install_yq.sh
	${MAKE} -f $(MK_PATH) all -j $(shell nproc ${CI:+--ignore 1}) V=

all: serial-targets \
	firecracker-tarball \
	kernel-tarball \
	qemu-tarball \
	shim-v2-tarball \
	virtiofsd-tarball

serial-targets:
	${MAKE} -f $(MK_PATH) -j 1 V= \
	rootfs-image-tarball \
	rootfs-initrd-tarball \
	cloud-hypervisor-tarball

%-tarball-build: $(MK_DIR)/dockerbuild/install_yq.sh
	$(call BUILD,$*)

cloud-hypervisor-tarball:
	${MAKE} $@-build

firecracker-tarball:
	${MAKE} $@-build

kernel-tarball:
	${MAKE} $@-build

kernel-experimental-tarball:
	${MAKE} $@-build


qemu-tarball:
	${MAKE} $@-build

rootfs-image-tarball:
	${MAKE} $@-build

rootfs-initrd-tarball:
	${MAKE} $@-build

shim-v2-tarball:
	${MAKE} $@-build

virtiofsd-tarball:
	${MAKE} $@-build

merge-builds:
	$(MK_DIR)/kata-deploy-merge-builds.sh build

install-tarball:
	tar -xvf ./kata-static.tar.xz -C /
