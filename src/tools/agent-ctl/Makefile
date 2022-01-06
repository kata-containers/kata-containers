# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

.DEFAULT_GOAL := default
default: build

build:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE)

clean:
	cargo clean

vendor:
	cargo vendor

test:

install:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo install --target $(TRIPLE) --path .

check: standard_rust_check

.PHONY: \
	build \
	check \
	clean \
	install \
	test \
	vendor
