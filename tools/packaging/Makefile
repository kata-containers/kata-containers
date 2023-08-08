#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#

MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
SED := sed
YQ := $(MK_DIR)/yq
VERSIONS_YAML_FILE := ../../versions.yaml
VERSION_FILE := ../../VERSION

export MK_DIR
export YQ
export VERSION_FILE
export VERSIONS_YAML_FILE

test-static-build:
	@make -f $(MK_DIR)/static-build/qemu/Makefile
	@make -f $(MK_DIR)/static-build/firecracker/Makefile

$(YQ):
	@bash -c "source scripts/lib.sh; install_yq $${MK_DIR}"

.PHONY: test-static-build
