#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
#

MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
.PHONY: test test-release-tools

test:
	@$(MK_DIR)/.ci/test.sh

test-release-tools:
	@$(MK_DIR)/release/tag_repos_test.sh

test-static-build:
	@make -f $(MK_DIR)/static-build/qemu/Makefile

test-packaging-tools:
	@$(MK_DIR)/build_from_docker.sh
