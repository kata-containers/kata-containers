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
	kernel-debug-tarball \
	kernel-dragonball-experimental-tarball \
	kernel-nvidia-gpu-tarball \
	kernel-tarball \
	nydus-tarball \
	ovmf-sev-tarball \
	ovmf-tdx-tarball \
	ovmf-tarball \
	qemu-snp-experimental-tarball \
	qemu-tdx-experimental-tarball \
	qemu-tarball \
	stratovirt-tarball \
	shim-v2-go-tarball \
	shim-v2-rust-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = rootfs-image-tarball \
	rootfs-image-confidential-tarball \
	rootfs-image-mariner-tarball \
	rootfs-initrd-confidential-tarball \
	rootfs-initrd-tarball \
	cloud-hypervisor-tarball
else ifeq ($(ARCH), s390x)
BASE_TARBALLS = serial-targets \
	kernel-debug-tarball \
	kernel-tarball \
	qemu-tarball \
	shim-v2-go-tarball \
	shim-v2-rust-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = rootfs-image-tarball \
	rootfs-initrd-tarball
else ifeq ($(ARCH), aarch64)
BASE_TARBALLS = serial-targets \
	kernel-cca-confidential-tarball \
	kernel-debug-tarball \
	kernel-tarball \
	qemu-tarball \
	qemu-cca-experimental-tarball \
	shim-v2-go-tarball \
	shim-v2-rust-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = rootfs-image-tarball \
	rootfs-image-confidential-tarball \
	rootfs-cca-confidential-image-tarball \
	rootfs-cca-confidential-initrd-tarball \
	rootfs-initrd-tarball
endif

PUBLISH_COMPONENT_TARBALLS = \
	kata-deploy-binary-tarball \
	nydus-snapshotter-for-coco-guest-pull-tarball

ifeq ($(ARCH), x86_64)

NVGPU_BASE_TARBALLS = \
	agent-tarball \
	busybox-tarball \
	coco-guest-components-tarball \
	$(PUBLISH_COMPONENT_TARBALLS) \
	kernel-nvidia-gpu-tarball \
	ovmf-sev-tarball \
	ovmf-tdx-tarball \
	ovmf-tarball \
	pause-image-tarball \
	qemu-snp-experimental-tarball \
	qemu-tdx-experimental-tarball \
	qemu-tarball \
	virtiofsd-tarball \
	serial-targets
else ifeq ($(ARCH), aarch64)
NVGPU_BASE_TARBALLS = \
	agent-tarball \
	busybox-tarball \
	coco-guest-components-tarball \
	$(PUBLISH_COMPONENT_TARBALLS) \
	kernel-nvidia-gpu-tarball \
	ovmf-tarball \
	pause-image-tarball \
	qemu-tarball \
	virtiofsd-tarball \
	serial-targets
endif
# Include kata-deploy static payload tarballs so `make kata-deploy-publish`
# can consume a single nvgpu bundle without rebuilding extra components.
NVGPU_FINAL_TARBALL_INPUTS = \
	kata-deploy-static-kata-deploy-binary.tar.zst \
	kata-deploy-static-nydus-snapshotter-for-coco-guest-pull.tar.zst \
	kata-static-kernel-nvidia-gpu.tar.zst \
	kata-static-ovmf-sev.tar.zst \
	kata-static-ovmf-tdx.tar.zst \
	kata-static-ovmf.tar.zst \
	kata-static-pause-image.tar.zst \
	kata-static-qemu-snp-experimental.tar.zst \
	kata-static-qemu-tdx-experimental.tar.zst \
	kata-static-qemu.tar.zst \
	kata-static-virtiofsd.tar.zst \
	kata-static-rootfs-image-nvidia-gpu.tar.zst \
	kata-static-rootfs-image-nvidia-gpu-confidential.tar.zst \
	kata-static-shim-v2-go.tar.zst \
	kata-static-shim-v2-rust.tar.zst


ifneq ($(NVGPU_BASE_TARBALLS),)
nvgpu-tarball:
	${MAKE} -f $(MK_PATH) kata-tarball \
		BASE_TARBALLS="$(NVGPU_BASE_TARBALLS)" \
		BASE_SERIAL_TARBALLS="rootfs-image-nvidia-gpu-tarball rootfs-image-nvidia-gpu-confidential-tarball" \
		FINAL_TARBALL_INPUTS="$(NVGPU_FINAL_TARBALL_INPUTS)" \
		FINAL_TARBALL_MERGE_MODE=passthrough \
		DEPS=
else
nvgpu-tarball:
	@echo "nvgpu-tarball: unsupported on $(ARCH)"; exit 1
endif

define BUILD
	$(MK_DIR)/kata-deploy-binaries-in-docker.sh $(if $(V),,-s) --build=$1
endef

define BUILD_KATA_DEPLOY_COMPONENT
	$(MK_DIR)/kata-deploy-build-components-tarballs.sh $1
endef

define DUMMY
	$(call BUILD,"dummy")
	mv $(MK_DIR)/build/kata-static-dummy.tar.zst $(MK_DIR)/build/kata-static-$(patsubst %-tarball,%,$1).tar.zst
endef

kata-tarball: | all-parallel merge-builds

copy-scripts-for-the-agent-build:
	${MK_DIR}/kata-deploy-copy-libseccomp-installer.sh "agent"

copy-scripts-for-the-tools-build:
	${MK_DIR}/kata-deploy-copy-libseccomp-installer.sh "tools"

all-parallel:
	${MAKE} -f $(MK_PATH) all -j $(shell nproc) --output-sync=target V=
	${MAKE} -f $(MK_PATH) shim-v2-go-tarball shim-v2-rust-tarball V=

all: ${BASE_TARBALLS}

serial-targets: $(filter-out serial-targets,$(BASE_TARBALLS))
	${MAKE} -f $(MK_PATH) -j 1 V= \
	${BASE_SERIAL_TARBALLS}

# Optional explicit allowlist for merge-builds input tarballs.
# Keep empty for normal targets; set explicitly for NVIDIA flow.
FINAL_TARBALL_INPUTS ?=

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

kernel-tarball:
	${MAKE} $@-build

kernel-debug-tarball:
	${MAKE} $@-build

kernel-cca-confidential-tarball:
	${MAKE} $@-build

nydus-tarball:
	${MAKE} $@-build

ovmf-sev-tarball:
	${MAKE} $@-build

ovmf-tdx-tarball:
	${MAKE} $@-build

ovmf-cca-tarball:
	${MAKE} $@-build

ovmf-tarball:
	${MAKE} $@-build

qemu-snp-experimental-tarball:
	${MAKE} $@-build

qemu-tdx-experimental-tarball:
	${MAKE} $@-build

qemu-cca-experimental-tarball:
	${MAKE} $@-build

qemu-tarball:
	${MAKE} $@-build

# DEPS is rebound per target below; prereqs expand at parse time, so each rule
# freezes the current DEPS. `make DEPS=` from the command line zeros all of them.
ifeq ($(FAKE_SE_IMAGE),true)
DEPS :=
else
DEPS := kernel-tarball rootfs-initrd-confidential-tarball
endif
boot-image-se-tarball: $(DEPS)
	${MAKE} $@-build

stratovirt-tarball:
	${MAKE} $@-build

DEPS := agent-tarball
rootfs-image-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball pause-image-tarball coco-guest-components-tarball kernel-tarball
rootfs-image-confidential-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball
rootfs-image-mariner-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball pause-image-tarball coco-guest-components-tarball kernel-tarball
rootfs-initrd-confidential-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball
rootfs-initrd-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball busybox-tarball kernel-nvidia-gpu-tarball
rootfs-image-nvidia-gpu-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball busybox-tarball pause-image-tarball coco-guest-components-tarball kernel-nvidia-gpu-tarball
rootfs-image-nvidia-gpu-confidential-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball pause-image-tarball coco-guest-components-tarball kernel-cca-confidential-tarball
rootfs-cca-confidential-image-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball pause-image-tarball coco-guest-components-tarball kernel-cca-confidential-tarball
rootfs-cca-confidential-initrd-tarball: $(DEPS)
	${MAKE} $@-build

shim-v2-go-tarball:
	${MAKE} $@-build

shim-v2-rust-tarball:
	${MAKE} $@-build

trace-forwarder-tarball: copy-scripts-for-the-tools-build
	${MAKE} $@-build

virtiofsd-tarball:
	${MAKE} $@-build

kata-deploy-binary-tarball:
	$(call BUILD_KATA_DEPLOY_COMPONENT,kata-deploy-binary)

nydus-snapshotter-for-coco-guest-pull-tarball:
	$(call BUILD_KATA_DEPLOY_COMPONENT,nydus-snapshotter-for-coco-guest-pull)

FINAL_TARBALL_MERGE_MODE ?= merge

merge-builds:
	$(MK_DIR)/kata-deploy-merge-builds.sh \
		build \
		"$(MK_DIR)/../../../../versions.yaml" \
		kata-static.tar.zst \
		"$(FINAL_TARBALL_INPUTS)" \
		"$(FINAL_TARBALL_MERGE_MODE)"

.PHONY: install-prebuilt-artifacts
install-prebuilt-artifacts:
	mv kata-artifacts $(MK_DIR)/build

install-tarball:
	tar --zstd -xf ./kata-static.tar.zst -C /

# Push the kata-deploy image and helm chart for the host arch. Caller is
# responsible for `docker login` / `helm registry login` before invoking.
# This target expects explicit REGISTRY/TAG/CHART_REGISTRY/CHART_VERSION args.
kata-deploy-publish:
	@set -eu; \
	tmpdir="$$(mktemp -d)"; \
	trap 'rm -rf "$$tmpdir"' EXIT; \
	tar --zstd -xf "$(CURDIR)/kata-static.tar.zst" -C "$$tmpdir"; \
	"$(MK_DIR)/kata-deploy-build-and-upload-payload.sh" "$(REGISTRY)" "$(TAG)" "$$tmpdir"; \
	"$(MK_DIR)/kata-deploy-build-and-upload-helm-chart.sh" \
		"$(REGISTRY)" "$(TAG)" "$(CHART_REGISTRY)" "$(CHART_VERSION)"
