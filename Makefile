# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# List of available components
COMPONENTS =

COMPONENTS += agent
COMPONENTS += runtime

# List of available tools
TOOLS =

TOOLS += agent-ctl
TOOLS += trace-forwarder

STANDARD_TARGETS = build check clean install test vendor

default: all

all: logging-crate-tests build

logging-crate-tests:
	make -C src/libs/logging

include utils.mk
include ./tools/packaging/kata-deploy/local-build/Makefile

# Create the rules
$(eval $(call create_all_rules,$(COMPONENTS),$(TOOLS),$(STANDARD_TARGETS)))

# Non-standard rules

generate-protocols:
	make -C src/agent generate-protocols

# Some static checks rely on generated source files of components.
static-checks: build
	bash ci/static-checks.sh

.PHONY: \
	all \
	binary-tarball \
	default \
	install-binary-tarball \
	logging-crate-tests \
	static-checks
