# Copyright (c) 2020-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# List of available components
COMPONENTS =

COMPONENTS += libs
COMPONENTS += agent
COMPONENTS += dragonball
COMPONENTS += runtime
COMPONENTS += runtime-rs

# List of available tools
TOOLS =

TOOLS += agent-ctl
TOOLS += kata-ctl
TOOLS += log-parser
TOOLS += runk
TOOLS += trace-forwarder

STANDARD_TARGETS = build check clean install static-checks-build test vendor

# Variables for the build-and-publish-kata-debug target
KATA_DEBUG_REGISTRY ?= ""
KATA_DEBUG_TAG ?= ""

default: all

include utils.mk
include ./tools/packaging/kata-deploy/local-build/Makefile

# Create the rules
$(eval $(call create_all_rules,$(COMPONENTS),$(TOOLS),$(STANDARD_TARGETS)))

# Non-standard rules

generate-protocols:
	make -C src/agent generate-protocols

# Some static checks rely on generated source files of components.
static-checks: static-checks-build
	bash tests/static-checks.sh github.com/kata-containers/kata-containers

docs-url-alive-check:
	bash ci/docs-url-alive-check.sh

build-and-publish-kata-debug:
	bash tools/packaging/kata-debug/kata-debug-build-and-upload-payload.sh ${KATA_DEBUG_REGISTRY} ${KATA_DEBUG_TAG} 

.PHONY: \
	all \
	kata-tarball \
	install-tarball \
	default \
	static-checks \
	docs-url-alive-check
