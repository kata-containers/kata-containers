#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#

MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
SED := sed
YQ := $(MK_DIR)/yq
SNAPCRAFT_FILE := ../../snap/snapcraft.yaml
VERSIONS_YAML_FILE := ../../versions.yaml
VERSION_FILE := ../../VERSION

export MK_DIR
export YQ
export SNAPCRAFT_FILE
export VERSION_FILE
export VERSIONS_YAML_FILE

test-static-build:
	@make -f $(MK_DIR)/static-build/qemu/Makefile
	@make -f $(MK_DIR)/static-build/firecracker/Makefile

$(YQ):
	@bash -c "source scripts/lib.sh; install_yq $${MK_DIR}"

snap: $(YQ)
	@if [ "$$(cat $(VERSION_FILE))" != "$$($(YQ) r $(SNAPCRAFT_FILE) version)" ]; then \
		>&2 echo "Warning: $(SNAPCRAFT_FILE) version is different to upstream $(VERSION_FILE) file"; \
	fi
	snapcraft -d

cmd-kata-pkgsync:
	@make -C $(MK_DIR)/cmd/kata-pkgsync

.PHONY: test-static-build snap cmd-kata-pkgsync
