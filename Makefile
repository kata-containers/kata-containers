# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# List of available components
COMPONENTS =

COMPONENTS += libs
COMPONENTS += agent
COMPONENTS += runtime
COMPONENTS += runtime-rs

# List of available tools
TOOLS =

TOOLS += agent-ctl
TOOLS += trace-forwarder
TOOLS += runk
TOOLS += log-parser

STANDARD_TARGETS = build check clean install test vendor

default: all

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

docs-url-alive-check:
	bash ci/docs-url-alive-check.sh

.PHONY: \
	all \
	binary-tarball \
	default \
	install-binary-tarball \
	static-checks \
	docs-url-alive-check


