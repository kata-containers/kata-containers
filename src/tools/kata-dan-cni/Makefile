# Copyright (c) 2026 Kata Containers community
#
# SPDX-License-Identifier: Apache-2.0

LIBC ?= gnu

include ../../../utils.mk

CARGO ?= cargo
INSTALL ?= install

PACKAGE := kata-dan-cni
BINARY := kata-dan-cni
TARGET_DIR = $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),../../../target)
BINARY_DIR = $(if $(findstring release,$(BUILD_TYPE)),release,debug)
RELEASE_FLAG = $(if $(findstring release,$(BUILD_TYPE)),--release)

CNI_BIN_DIR ?= /opt/cni/bin
DESTDIR ?=

.DEFAULT_GOAL := default
default: build
all: build

build:
	RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" $(CARGO) build --locked -p $(PACKAGE) --bins --target $(TRIPLE) $(RELEASE_FLAG)

release:
	$(MAKE) build BUILD_TYPE=release

fmt:
	$(CARGO) fmt $(PACKAGE_FLAGS) -- --check

clippy:
	$(CARGO) clippy $(PACKAGE_FLAGS) --all-targets --all-features --release --locked -- -D warnings

check: standard_rust_check test

test:
	RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" $(CARGO) test $(PACKAGE_FLAGS) --all-targets --all-features --locked --target $(TRIPLE) $(RELEASE_FLAG)

test-integration:
	CARGO="$(CARGO)" TRIPLE="$(TRIPLE)" BUILD_TYPE="$(BUILD_TYPE)" RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" bash tests/run-integration.sh

clean:
	$(CARGO) clean $(PACKAGE_FLAGS)

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

install: build
	$(INSTALL) -D -m 0755 \
		"$(TARGET_DIR)/$(TRIPLE)/$(BINARY_DIR)/$(BINARY)" \
		"$(DESTDIR)$(CNI_BIN_DIR)/$(BINARY)"

.PHONY: default all build release fmt clippy check test test-integration clean static-checks-build install
