# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

MK_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MK_DIR := $(dir $(MK_PATH))

# Verbose build
V := 1

ARCH := $(shell uname -m)

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
	qemu-no-shared-fs-tarball \
	qemu-snp-experimental-tarball \
	qemu-tdx-experimental-tarball \
	qemu-tarball \
	stratovirt-tarball \
	shim-v2-go-tarball \
	shim-v2-rust-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = openvmm-tarball \
	rootfs-image-tarball \
	rootfs-image-confidential-tarball \
	rootfs-image-coco-extension-tarball \
	rootfs-image-devkit-extension-tarball \
	rootfs-image-mariner-tarball \
	rootfs-image-nvidia-gpu-tarball \
	rootfs-image-nvidia-gpu-confidential-tarball \
	rootfs-image-nvidia-tarball \
	rootfs-image-nvidia-gpu-extension-tarball \
	rootfs-initrd-confidential-tarball \
	rootfs-initrd-tarball \
	cloud-hypervisor-tarball
else ifeq ($(ARCH), s390x)
BASE_TARBALLS = serial-targets \
	boot-image-se-tarball \
	boot-image-se-runtime-rs-tarball \
	kernel-debug-tarball \
	kernel-tarball \
	qemu-tarball \
	shim-v2-go-tarball \
	shim-v2-rust-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = rootfs-image-tarball \
	rootfs-image-confidential-tarball \
	rootfs-image-coco-extension-tarball \
	rootfs-initrd-confidential-tarball \
	rootfs-initrd-tarball
else ifeq ($(ARCH), aarch64)
BASE_TARBALLS = serial-targets \
	firecracker-tarball \
	kernel-debug-tarball \
	kernel-dragonball-experimental-tarball \
	kernel-tarball \
	nydus-tarball \
	ovmf-tarball \
	qemu-no-shared-fs-tarball \
	qemu-tarball \
	shim-v2-go-tarball \
	shim-v2-rust-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = openvmm-tarball \
	rootfs-image-tarball \
	rootfs-image-confidential-tarball \
	rootfs-image-coco-extension-tarball \
	rootfs-image-devkit-extension-tarball \
	rootfs-image-nvidia-gpu-tarball \
	rootfs-image-nvidia-tarball \
	rootfs-image-nvidia-gpu-extension-tarball \
	rootfs-initrd-tarball \
	cloud-hypervisor-tarball
else ifeq ($(ARCH), ppc64le)
BASE_TARBALLS = serial-targets \
	kernel-tarball \
	qemu-tarball \
	shim-v2-go-tarball \
	virtiofsd-tarball
BASE_SERIAL_TARBALLS = rootfs-initrd-tarball
endif

ifeq ($(ARCH), ppc64le)
SHIM_TARBALLS = shim-v2-go-tarball
else
SHIM_TARBALLS = shim-v2-go-tarball shim-v2-rust-tarball
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
	qemu-no-shared-fs-tarball \
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
	qemu-no-shared-fs-tarball \
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
	kata-static-qemu-no-shared-fs.tar.zst \
	kata-static-qemu-snp-experimental.tar.zst \
	kata-static-qemu-tdx-experimental.tar.zst \
	kata-static-qemu.tar.zst \
	kata-static-virtiofsd.tar.zst \
	kata-static-rootfs-image-nvidia-gpu.tar.zst \
	kata-static-rootfs-image-nvidia-gpu-confidential.tar.zst \
	kata-static-rootfs-image-nvidia.tar.zst \
	kata-static-rootfs-image-nvidia-gpu-extension.tar.zst \
	kata-static-shim-v2-go.tar.zst \
	kata-static-shim-v2-rust.tar.zst


ifneq ($(NVGPU_BASE_TARBALLS),)
nvgpu-tarball:
	${MAKE} -f $(MK_PATH) kata-tarball \
		BASE_TARBALLS="$(NVGPU_BASE_TARBALLS)" \
		BASE_SERIAL_TARBALLS="rootfs-image-nvidia-gpu-tarball rootfs-image-nvidia-gpu-confidential-tarball rootfs-image-nvidia-tarball" \
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

ifneq ($(filter $(ARCH),x86_64 aarch64 s390x ppc64le),)
KATA_STATIC_FINAL_TARBALL_INPUTS := $(shell $(MK_DIR)/kata-release-tarball-inputs.sh kata-static $(ARCH) | tr '\n' ' ')
KATA_GO_STATIC_FINAL_TARBALL_INPUTS := $(shell $(MK_DIR)/kata-release-tarball-inputs.sh kata-go-static $(ARCH) | tr '\n' ' ')
endif

ifneq ($(KATA_STATIC_FINAL_TARBALL_INPUTS),)
kata-tarball: FINAL_TARBALL_INPUTS=$(KATA_STATIC_FINAL_TARBALL_INPUTS)
kata-tarball: FINAL_TARBALL_NAME=kata-static.tar.zst
kata-tarball: | all-parallel
	$(MAKE) -f $(MK_PATH) merge-builds \
		FINAL_TARBALL_NAME="$(FINAL_TARBALL_NAME)" \
		FINAL_TARBALL_INPUTS="$(FINAL_TARBALL_INPUTS)" \
		FINAL_TARBALL_MERGE_MODE="$(FINAL_TARBALL_MERGE_MODE)"
else
kata-tarball: | all-parallel merge-builds
endif

ifneq ($(KATA_GO_STATIC_FINAL_TARBALL_INPUTS),)
kata-go-static-tarball: FINAL_TARBALL_INPUTS=$(KATA_GO_STATIC_FINAL_TARBALL_INPUTS)
kata-go-static-tarball: FINAL_TARBALL_NAME=kata-go-static.tar.zst
kata-go-static-tarball: | all-parallel
	$(MAKE) -f $(MK_PATH) merge-builds \
		FINAL_TARBALL_NAME="$(FINAL_TARBALL_NAME)" \
		FINAL_TARBALL_INPUTS="$(FINAL_TARBALL_INPUTS)" \
		FINAL_TARBALL_MERGE_MODE="$(FINAL_TARBALL_MERGE_MODE)"
else
kata-go-static-tarball:
	@echo "kata-go-static-tarball: unsupported on $(ARCH)"; exit 1
endif

copy-scripts-for-the-agent-build:
	${MK_DIR}/kata-deploy-copy-libseccomp-installer.sh "agent"

copy-scripts-for-the-tools-build:
	${MK_DIR}/kata-deploy-copy-libseccomp-installer.sh "tools"

all-parallel:
	${MAKE} -f $(MK_PATH) all -j $(shell nproc) --output-sync=target V=
	${MAKE} -f $(MK_PATH) $(SHIM_TARBALLS) V=

all: ${BASE_TARBALLS}

serial-targets: $(filter-out serial-targets,$(BASE_TARBALLS))
	${MAKE} -f $(MK_PATH) -j 1 V= \
	${BASE_SERIAL_TARBALLS}

# Optional explicit allowlist for merge-builds input tarballs.
# Release and NVIDIA targets set this to keep their contents deterministic.
FINAL_TARBALL_INPUTS ?=
FINAL_TARBALL_NAME ?= kata-static.tar.zst

%-tarball-build:
	$(call BUILD,$*)

agent-tarball: copy-scripts-for-the-agent-build
	${MAKE} $@-build

agent-ctl-tarball: copy-scripts-for-the-tools-build
	${MAKE} $@-build

BUSYBOX_CONF_FILE ?= busybox.nvidia.fragment
busybox-tarball:
	${MAKE} BUSYBOX_CONF_FILE=${BUSYBOX_CONF_FILE} $@-build

coco-guest-components-tarball:
	${MAKE} $@-build

cloud-hypervisor-tarball:
	${MAKE} $@-build

openvmm-tarball:
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

nydus-tarball:
	${MAKE} $@-build

ovmf-sev-tarball:
	${MAKE} $@-build

ovmf-tdx-tarball:
	${MAKE} $@-build

ovmf-tarball:
	${MAKE} $@-build

qemu-no-shared-fs-tarball:
	${MAKE} $@-build

qemu-microvm-tarball:
	${MAKE} $@-build

qemu-snp-experimental-tarball:
	${MAKE} $@-build

qemu-tdx-experimental-tarball:
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

ifeq ($(FAKE_SE_IMAGE),true)
DEPS :=
else
DEPS := kernel-tarball rootfs-initrd-tarball rootfs-image-coco-extension-tarball
endif
boot-image-se-runtime-rs-tarball: $(DEPS)
	${MAKE} $@-build

stratovirt-tarball:
	${MAKE} $@-build

DEPS := agent-tarball
rootfs-image-tarball: $(DEPS)
	${MAKE} $@-build

DEPS := agent-tarball pause-image-tarball coco-guest-components-tarball kernel-tarball
rootfs-image-confidential-tarball: $(DEPS)
	${MAKE} $@-build

rootfs-image-coco-extension-tarball:
	${MAKE} $@-build

# Self-contained debug rootfs (no kata-agent, kernel or driver tree), so it has
# no build-time dependency on any other target.
DEPS :=
rootfs-image-devkit-extension-tarball: $(DEPS)
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

# The driver-agnostic nvidia base is still carved from the chiseled monolith tree,
# so it keeps the monolith's (non-confidential) build dependencies.
DEPS := agent-tarball busybox-tarball kernel-nvidia-gpu-tarball
rootfs-image-nvidia-tarball: $(DEPS)
	${MAKE} $@-build

# The gpu extension is assembled directly from the package-installed rootfs and
# ships neither the agent nor NVRC/busybox; it only needs the Kata NVIDIA modules.
DEPS := kernel-nvidia-gpu-tarball
rootfs-image-nvidia-gpu-extension-tarball: $(DEPS)
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
		"$(FINAL_TARBALL_NAME)" \
		"$(FINAL_TARBALL_INPUTS)" \
		"$(FINAL_TARBALL_MERGE_MODE)"

.PHONY: install-prebuilt-artifacts
install-prebuilt-artifacts:
	mv kata-artifacts $(MK_DIR)/build

# The payload is unpacked straight onto this machine's root, so vet it first:
# a stray etc/resolv.conf in there would replace the local resolver.
install-tarball:
	$(MK_DIR)/../../scripts/assert-tarball-host-safe.sh ./kata-static.tar.zst
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
