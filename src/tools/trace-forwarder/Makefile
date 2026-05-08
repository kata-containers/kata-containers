# Copyright (c) 2020-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

.DEFAULT_GOAL := default
default: build

build:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build -p kata-trace-forwarder --target $(TRIPLE) --$(BUILD_TYPE)

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

clean:
	cargo clean

test:
	@cargo test -p kata-trace-forwarder -- --nocapture

install:

check: standard_rust_check

.PHONY: \
	build \
	check \
	clean \
	install \
	test
