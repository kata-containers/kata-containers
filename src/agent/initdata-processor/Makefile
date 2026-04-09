# Copyright (c) 2026 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#
.PHONY: all build test clean install systemd-install

PREFIX ?= /usr
BINDIR ?= $(PREFIX)/bin
SYSTEMD_DIR ?= /usr/lib/systemd/system

include ../../../utils.mk

all: build

build:
	RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) $(if $(findstring release,$(BUILD_TYPE)),--release)

test:
	cargo test

clean:
	CARGO_TARGET_DIR=./target cargo clean

install: build
	install -D -m 755 target/release/initdata-processor $(DESTDIR)$(BINDIR)/initdata-processor

systemd-install:
	install -D -m 644 systemd-services/initdata-processor.service $(DESTDIR)$(SYSTEMD_DIR)/initdata-processor.service
