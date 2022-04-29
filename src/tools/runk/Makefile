# Copyright 2021-2022 Sony Group Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

TARGET = runk
TARGET_PATH = target/$(TRIPLE)/$(BUILD_TYPE)/$(TARGET)

AGENT_TARGET = oci-kata-agent
AGENT_TARGET_PATH = target/$(TRIPLE)/$(BUILD_TYPE)/$(AGENT_TARGET)
AGENT_SOURCE_PATH = ../../agent

# BINDIR is a directory for installing executable programs
BINDIR := /usr/local/bin

.DEFAULT_GOAL := default
default: build

build: build-agent build-runk

build-agent:
	make -C $(AGENT_SOURCE_PATH) STANDARD_OCI_RUNTIME=yes

build-runk:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE)

install: install-agent install-runk

install-agent:
	install -D $(AGENT_SOURCE_PATH)/$(AGENT_TARGET_PATH) $(BINDIR)/$(AGENT_TARGET)

install-runk:
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
	build-agent \
	build-runk \
	install \
	install-agent \
	install-runk \
	clean \
	clippy \
	format \
	vendor \
	test \
	check \
