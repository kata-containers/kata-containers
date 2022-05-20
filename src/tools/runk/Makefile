# Copyright 2021-2022 Sony Group Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

TARGET = runk
TARGET_PATH = target/$(TRIPLE)/$(BUILD_TYPE)/$(TARGET)

# BINDIR is a directory for installing executable programs
BINDIR := /usr/local/bin

.DEFAULT_GOAL := default
default: build

build:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE)

install:
	install -D $(TARGET_PATH) $(BINDIR)/$(TARGET)

clean:
	cargo clean

vendor:
	cargo vendor

test:
	cargo test --all --target $(TRIPLE) -- --nocapture

check: standard_rust_check

.PHONY: \
	build \
	install \
	clean \
	clippy \
	format \
	vendor \
	test \
	check \
