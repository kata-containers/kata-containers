#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#

MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
.PHONY: bump-kata-version

NEW_VERSION :=
TARGET_BRANCH ?= "main"

# Run update-repository-version.sh
# $1 : repository to bump
define BUMP_REPO
	@echo "Create PR for $1 version $(NEW_VERSION)"
	@$(MK_DIR)/update-repository-version.sh -p $(NEW_VERSION) $(TARGET_BRANCH)
endef

bump-kata-version: $(REPOS)
ifeq ($(and $(NEW_VERSION),$(TARGET_BRANCH)),)
	$(error Either NEW_VERSION or TARGET_BRANCH variable is empty, provide both)
else
	$(call BUMP_REPO)
endif
