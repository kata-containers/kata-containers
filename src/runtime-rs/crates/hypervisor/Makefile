# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# On-demand regeneration of the vendored OpenVMM TTRPC `vmservice` proto.
#
# This mirrors the Cloud Hypervisor client workflow
# (src/runtime/virtcontainers/pkg/cloud-hypervisor/Makefile): it is a manual
# developer target, NOT part of the build. It fetches the proto at the OpenVMM
# revision pinned in versions.yaml. The fetched proto is checked in and compiled
# by build.rs (which regenerates the Rust bindings automatically on change), so
# normal builds need neither network nor this target.
#
# Usage:
#   make -C src/runtime-rs/crates/hypervisor update-proto

MK_DIR := $(dir $(realpath $(firstword $(MAKEFILE_LIST))))
VERSIONS_FILE := $(MK_DIR)../../../../versions.yaml
YQ_INSTALLER := $(MK_DIR)../../../../ci/install_yq.sh
PROTO_DIR := $(MK_DIR)src/openvmm/protos
OPENVMM_RAW_BASE := https://raw.githubusercontent.com/microsoft/openvmm
VMSERVICE_PROTO_PATH := openvmm/openvmm_ttrpc_vmservice/src/vmservice.proto

YQ := $(shell command -v yq 2>/dev/null)

.PHONY: update-proto
update-proto:
ifndef YQ
	$(YQ_INSTALLER)
endif
	@openvmm_version=$$(yq '.assets.hypervisor.openvmm.version' "$(VERSIONS_FILE)"); \
	if [ -z "$$openvmm_version" ] || [ "$$openvmm_version" = "null" ]; then \
		echo "error: .assets.hypervisor.openvmm.version missing from $(VERSIONS_FILE)"; \
		exit 1; \
	fi; \
	echo "fetching vmservice.proto from openvmm@$$openvmm_version"; \
	curl -fSL "$(OPENVMM_RAW_BASE)/$$openvmm_version/$(VMSERVICE_PROTO_PATH)" \
		-o "$(PROTO_DIR)/vmservice.proto"
