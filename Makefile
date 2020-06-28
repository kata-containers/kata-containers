# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# List of available components
COMPONENTS =

COMPONENTS += agent
COMPONENTS += runtime
COMPONENTS += trace-forwarder

# List of available tools
TOOLS =

TOOLS += agent-ctl

STANDARD_TARGETS = build check clean install test

include utils.mk

all: build

# Create the rules
$(eval $(call create_all_rules,$(COMPONENTS),$(TOOLS),$(STANDARD_TARGETS)))

# Non-standard rules

generate-protocols:
	make -C src/agent generate-protocols

.PHONY: all default
