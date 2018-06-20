#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#

MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
.PHONY: bump-kata-version

NEW_VERSION :=

# Run update-repository-version.sh
# $1 : repository to bump
define BUMP_REPO
	@echo "Create PR for $1 version $(NEW_VERSION)"
	@$(MK_DIR)/update-repository-version.sh -p $1 $(NEW_VERSION)
endef

bump-kata-version: $(REPOS)
ifeq ($(NEW_VERSION),)
	$(error NEW_VERSION variable is empty, provide a version)
else
	$(call BUMP_REPO,agent)
	$(call BUMP_REPO,ksm-throttler)
	$(call BUMP_REPO,osbuilder)
	$(call BUMP_REPO,proxy)
	$(call BUMP_REPO,runtime)
	$(call BUMP_REPO,shim)
endif
