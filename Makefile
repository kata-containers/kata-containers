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
export VERSION_FILE
export VERSIONS_YAML_FILE

test:
	@$(MK_DIR)/.ci/test.sh

test-release-tools:
	@$(MK_DIR)/release/tag_repos_test.sh

test-static-build:
	@make -f $(MK_DIR)/static-build/qemu/Makefile

test-packaging-tools:
	@$(MK_DIR)/obs-packaging/build_from_docker.sh

$(YQ):
	@bash -c "source .ci/lib.sh; install_yq $${MK_DIR}"

$(VERSION_FILE):
	@curl -sO $(VERSION_FILE_URL)

$(VERSIONS_YAML_FILE):
	@curl -sO $(VERSIONS_YAML_FILE_URL)

$(SNAPCRAFT_FILE): %: %.in Makefile $(YQ) $(VERSIONS_YAML_FILE) $(VERSION_FILE)
	$(SED) \
		-e "s|@KATA_RUNTIME_VERSION@|$$(cat $${VERSION_FILE})|g" \
		-e "s|@KATA_PROXY_VERSION@|$$(cat $${VERSION_FILE})|g" \
		-e "s|@KATA_SHIM_VERSION@|$$(cat $${VERSION_FILE})|g" \
		-e "s|@KSM_THROTTLER_VERSION@|$$(cat $${VERSION_FILE})|g" \
		-e "s|@QEMU_LITE_BRANCH@|$$($${YQ} r $${VERSIONS_YAML_FILE} assets.hypervisor.qemu-lite.branch)|g" \
		-e "s|@KERNEL_URL@|$$($${YQ} r $${VERSIONS_YAML_FILE} assets.kernel.url)|g" \
		-e "s|@KERNEL_VERSION@|$$($${YQ} r $${VERSIONS_YAML_FILE} assets.kernel.version | tr -d v)|g" \
		-e "s|@GO_VERSION@|$$($${YQ} r $${VERSIONS_YAML_FILE} languages.golang.meta.newest-version)|g" \
		$< > $@

snap: $(SNAPCRAFT_FILE)
	snapcraft -d

clean:
	rm $(SNAPCRAFT_FILE)

.PHONY: test test-release-tools test-static-build test-packaging-tools snap clean \
	$(VERSION_FILE) $(VERSIONS_YAML_FILE)
