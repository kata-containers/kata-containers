# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

MK_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

.DEFAULT_GOAL := default
default: build

build: logging-crate-tests
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE)

logging-crate-tests:
	make -C $(CWD)/../../libs/logging

clean:
	cargo clean

vendor:
	cargo vendor

test:

include $(MK_DIR)/../../../utils.mk

install:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo install --target $(TRIPLE) --path .

check: standard_rust_check

.PHONY: \
	build \
	check \
	clean \
	install \
	logging-crate-tests \
	test \
	vendor
