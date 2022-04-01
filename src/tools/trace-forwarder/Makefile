# Copyright (c) 2020-2021 Intel Corporation
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
	@cargo test --all -- --nocapture

include $(MK_DIR)/../../../utils.mk

install:

check: standard_rust_check

.PHONY: \
	build \
	check \
	clean \
	install \
	logging-crate-tests \
	test \
	vendor
