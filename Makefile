#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#

MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
SED := sed
YQ := $(MK_DIR)/yq
SNAPCRAFT_FILE := snap/snapcraft.yaml
VERSIONS_YAML_FILE := versions.yaml
VERSIONS_YAML_FILE_URL := https://raw.githubusercontent.com/kata-containers/runtime/master/versions.yaml
VERSION_FILE := VERSION
VERSION_FILE_URL := https://raw.githubusercontent.com/kata-containers/runtime/master/VERSION

export MK_DIR
export YQ
export SNAPCRAFT_FILE
export VERSION_FILE
export VERSIONS_YAML_FILE

test:
	@$(MK_DIR)/.ci/test.sh

test-release-tools:
	@$(MK_DIR)/release/tag_repos_test.sh
	@$(MK_DIR)/release/update-repository-version_test.sh

test-static-build:
	@make -f $(MK_DIR)/static-build/qemu/Makefile
	@make -f $(MK_DIR)/static-build/firecracker/Makefile

test-packaging-tools:
	@$(MK_DIR)/obs-packaging/build_from_docker.sh

$(YQ):
	@bash -c "source scripts/lib.sh; install_yq $${MK_DIR}"

$(VERSION_FILE):
	@curl -sO $(VERSION_FILE_URL)

$(VERSIONS_YAML_FILE):
	@curl -sO $(VERSIONS_YAML_FILE_URL)

snap: $(YQ) $(VERSION_FILE)
	@if [ "$$(cat $(VERSION_FILE))" != "$$($(YQ) r $(SNAPCRAFT_FILE) version)" ]; then \
		>&2 echo "Warning: $(SNAPCRAFT_FILE) version is different to upstream $(VERSION_FILE) file"; \
	fi
	snapcraft -d

obs-test:
	@$(MK_DIR)/tests/run_obs_testing.sh

cmd-kata-pkgsync:
	@make -C $(MK_DIR)/cmd/kata-pkgsync

.PHONY: test test-release-tools test-static-build test-packaging-tools snap cmd-kata-pkgsync \
	$(VERSION_FILE) $(VERSIONS_YAML_FILE)
