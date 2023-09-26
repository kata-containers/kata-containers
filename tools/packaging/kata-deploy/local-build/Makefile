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
	qemu-tarball \
	shim-v2-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = agent-opa-tarball \
	kernel-nvidia-gpu-tarball \
	rootfs-nvidia-gpu-initrd-tarball 
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

kata-tarball: | all-parallel merge-builds

$(MK_DIR)/dockerbuild/install_yq.sh:
	$(MK_DIR)/kata-deploy-copy-yq-installer.sh

copy-scripts-for-the-agent-build:
	${MK_DIR}/kata-deploy-copy-libseccomp-installer.sh

all-parallel: $(MK_DIR)/dockerbuild/install_yq.sh
	${MAKE} -f $(MK_PATH) all -j $(shell nproc ${CI:+--ignore 1}) V=

all: ${BASE_TARBALLS}

serial-targets:
	${MAKE} -f $(MK_PATH) -j 1 V= \
	${BASE_SERIAL_TARBALLS}

%-tarball-build: $(MK_DIR)/dockerbuild/install_yq.sh
	$(call BUILD,$*)

agent-tarball: copy-scripts-for-the-agent-build
	${MAKE} $@-build

agent-opa-tarball: copy-scripts-for-the-agent-build
	${MAKE} $@-build

agent-ctl-tarball:
	${MAKE} $@-build

coco-guest-components-tarball:
	${MAKE} $@-build

cloud-hypervisor-tarball:
	${MAKE} $@-build

cloud-hypervisor-glibc-tarball:
	${MAKE} $@-build

firecracker-tarball:
	${MAKE} $@-build

genpolicy-tarball:
	${MAKE} $@-build

pause-image-tarball:
	${MAKE} $@-build

kata-ctl-tarball:
	${MAKE} $@-build

kata-manager-tarball:
	${MAKE} $@-build

kernel-dragonball-experimental-tarball:
	${MAKE} $@-build

kernel-nvidia-gpu-tarball:
	${MAKE} $@-build

kernel-nvidia-gpu-confidential-tarball:
	${MAKE} $@-build

kernel-headers-nvidia-gpu-tarball: kernel-nvidia-gpu-tarball
	${MAKE} $@-build

kernel-headers-nvidia-gpu-confidential-tarball: kernel-nvidia-gpu-confidential-tarball
	${MAKE} $@-build

kernel-nvidia-gpu-snp-tarball:
	${MAKE} $@-build

kernel-nvidia-gpu-tdx-experimental-tarball:
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

boot-image-se-tarball: kernel-tarball rootfs-initrd-tarball
	${MAKE} $@-build

qemu-tdx-experimental-tarball:
	${MAKE} $@-build

stratovirt-tarball:
	${MAKE} $@-build

rootfs-image-tarball: agent-tarball
	${MAKE} $@-build

rootfs-image-confidential-tarball: agent-opa-tarball pause-image-tarball coco-guest-components-tarball kernel-confidential-tarball
	${MAKE} $@-build

rootfs-initrd-mariner-tarball: agent-opa-tarball
	${MAKE} $@-build

rootfs-initrd-confidential-tarball: agent-opa-tarball pause-image-tarball coco-guest-components-tarball kernel-confidential-tarball
	${MAKE} $@-build

rootfs-initrd-tarball: agent-tarball
	${MAKE} $@-build

rootfs-nvidia-gpu-image-tarball: #kernel-headers-nvidia-gpu-tarball
	${MAKE} $@-build

rootfs-nvidia-gpu-initrd-tarball: #kernel-headers-nvidia-gpu-tarball
	${MAKE} $@-build

rootfs-nvidia-gpu-confidential-image-tarball: #agent-opa-tarball pause-image-tarball coco-guest-components-tarball kernel-nvidia-gpu-confidential-tarball
	${MAKE} $@-build

rootfs-nvidia-gpu-confidential-initrd-tarball: #agent-opa-tarball pause-image-tarball coco-guest-components-tarball kernel-nvidia-gpu-confidential-tarball
	${MAKE} $@-build

rootfs-initrd-tarball:
	${MAKE} $@-build

runk-tarball:
	${MAKE} $@-build

shim-v2-tarball:
	${MAKE} $@-build

tdvf-tarball:
	${MAKE} $@-build

trace-forwarder-tarball:
	${MAKE} $@-build

virtiofsd-tarball:
	${MAKE} $@-build

merge-builds:
	$(MK_DIR)/kata-deploy-merge-builds.sh build "./versions.yaml"

install-tarball:
	tar -xf ./kata-static.tar.xz -C /
