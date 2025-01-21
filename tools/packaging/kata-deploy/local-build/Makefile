# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

MK_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MK_DIR := $(dir $(MK_PATH))

# Verbose build
V := 1

ifeq ($(CROSS_BUILD),)
	CROSS_BUILD = false
endif

ifeq ($(CROSS_BUILD),false)
	ARCH := $(shell uname -m)
endif

ifeq ($(ARCH), x86_64)
BASE_TARBALLS = serial-targets \
	firecracker-tarball \
	kernel-confidential-tarball \
	kernel-dragonball-experimental-tarball \
	kernel-nvidia-gpu-tarball \
	kernel-nvidia-gpu-confidential-tarball \
	kernel-tarball \
	nydus-tarball \
	ovmf-sev-tarball \
	ovmf-tarball \
	qemu-snp-experimental-tarball \
	qemu-tarball \
	stratovirt-tarball \
	shim-v2-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = rootfs-image-tarball \
	rootfs-image-confidential-tarball \
	rootfs-image-mariner-tarball \
	rootfs-initrd-confidential-tarball \
	rootfs-initrd-tarball \
	cloud-hypervisor-tarball \
	cloud-hypervisor-glibc-tarball
else ifeq ($(ARCH), s390x)
BASE_TARBALLS = serial-targets \
	kernel-tarball \
	qemu-tarball \
	shim-v2-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = rootfs-image-tarball \
	rootfs-initrd-tarball
endif

define BUILD
	$(MK_DIR)/kata-deploy-binaries-in-docker.sh $(if $(V),,-s) --build=$1
endef

define DUMMY
	$(call BUILD,"dummy")
	mv $(MK_DIR)/build/kata-static-dummy.tar.xz $(MK_DIR)/build/kata-static-$(patsubst %-tarball,%,$1).tar.xz
endef

kata-tarball: | all-parallel merge-builds

copy-scripts-for-the-agent-build:
	${MK_DIR}/kata-deploy-copy-libseccomp-installer.sh "agent"

copy-scripts-for-the-tools-build:
	${MK_DIR}/kata-deploy-copy-libseccomp-installer.sh "tools"

all-parallel:
	${MAKE} -f $(MK_PATH) all -j $(shell nproc) V=

all: ${BASE_TARBALLS}

serial-targets:
	${MAKE} -f $(MK_PATH) -j 1 V= \
	${BASE_SERIAL_TARBALLS}

%-tarball-build:
	$(call BUILD,$*)

agent-tarball: copy-scripts-for-the-agent-build
	${MAKE} $@-build

agent-ctl-tarball: copy-scripts-for-the-tools-build
	${MAKE} $@-build

BUSYBOX_CONF_FILE ?= busybox.nvidia.conf
busybox-tarball: 
	${MAKE} BUSYBOX_CONF_FILE=${BUSYBOX_CONF_FILE} $@-build
	
coco-guest-components-tarball:
	${MAKE} $@-build

cloud-hypervisor-tarball:
	${MAKE} $@-build

cloud-hypervisor-glibc-tarball:
	${MAKE} $@-build

csi-kata-directvolume-tarball: copy-scripts-for-the-tools-build
	${MAKE} $@-build

firecracker-tarball:
	${MAKE} $@-build

genpolicy-tarball: copy-scripts-for-the-tools-build
	${MAKE} $@-build

pause-image-tarball:
	${MAKE} $@-build

kata-ctl-tarball: copy-scripts-for-the-tools-build
	${MAKE} $@-build

kata-manager-tarball:
	${MAKE} $@-build

kernel-nvidia-gpu-dragonball-experimental-tarball:
	${MAKE} $@-build

kernel-dragonball-experimental-tarball:
	${MAKE} $@-build

kernel-nvidia-gpu-tarball:
	${MAKE} $@-build

kernel-nvidia-gpu-confidential-tarball:
	${MAKE} $@-build

kernel-tarball:
	${MAKE} $@-build

kernel-confidential-tarball:
	${MAKE} $@-build

nydus-tarball:
	${MAKE} $@-build

ovmf-sev-tarball:
	${MAKE} $@-build

ovmf-tarball:
	${MAKE} $@-build

qemu-snp-experimental-tarball:
	${MAKE} $@-build

qemu-tarball:
	${MAKE} $@-build

boot-image-se-tarball: kernel-confidential-tarball rootfs-initrd-confidential-tarball
	${MAKE} $@-build

stratovirt-tarball:
	${MAKE} $@-build

rootfs-image-tarball: agent-tarball
	${MAKE} $@-build

rootfs-image-confidential-tarball: agent-tarball pause-image-tarball coco-guest-components-tarball kernel-confidential-tarball
	${MAKE} $@-build

rootfs-image-mariner-tarball: agent-tarball
	${MAKE} $@-build

rootfs-initrd-confidential-tarball: agent-tarball pause-image-tarball coco-guest-components-tarball kernel-confidential-tarball
	${MAKE} $@-build

rootfs-initrd-tarball: agent-tarball
	${MAKE} $@-build

runk-tarball: copy-scripts-for-the-tools-build
	${MAKE} $@-build
rootfs-nvidia-gpu-image-tarball: agent-tarball busybox-tarball kernel-nvidia-gpu-tarball
	${MAKE} $@-build

rootfs-nvidia-gpu-initrd-tarball: agent-tarball busybox-tarball kernel-nvidia-gpu-tarball
	${MAKE} $@-build

rootfs-nvidia-gpu-confidential-image-tarball: agent-tarball busybox-tarball pause-image-tarball coco-guest-components-tarball kernel-nvidia-gpu-confidential-tarball
	${MAKE} $@-build

rootfs-nvidia-gpu-confidential-initrd-tarball: agent-tarball busybox-tarball pause-image-tarball coco-guest-components-tarball kernel-nvidia-gpu-confidential-tarball
	${MAKE} $@-build

shim-v2-tarball:
	${MAKE} $@-build

trace-forwarder-tarball: copy-scripts-for-the-tools-build
	${MAKE} $@-build

virtiofsd-tarball:
	${MAKE} $@-build

merge-builds:
	$(MK_DIR)/kata-deploy-merge-builds.sh build "$(MK_DIR)/../../../../versions.yaml"

install-tarball:
	tar -xf ./kata-static.tar.xz -C /
