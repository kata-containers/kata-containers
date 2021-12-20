# Copyright (c) 2020-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

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

install:

check:

.PHONY: \
	build \
	check \
	clean \
	install \
	logging-crate-tests \
	test \
	vendor
