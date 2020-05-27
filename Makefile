# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

default: runtime agent

runtime:
	make -C src/runtime

agent:
	make -C src/agent

test-runtime:
	make -C src/runtime test

test-agent:
	make -C src/agent check

test: test-runtime test-agent

generate-protocols:
	make -C src/agent generate-protocols
