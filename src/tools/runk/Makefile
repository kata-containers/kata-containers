# Copyright 2021-2022 Sony Group Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# LIBC=musl|gnu (default: gnu)
LIBC ?= gnu

include ../../../utils.mk

ifeq ($(ARCH), ppc64le)
     override ARCH = powerpc64le
endif

TARGET = runk
TARGET_PATH = target/$(TRIPLE)/$(BUILD_TYPE)/$(TARGET)
AGENT_SOURCE_PATH = ../../agent

EXTRA_RUSTFEATURES :=

# Define if runk enables seccomp support (default: yes)
SECCOMP := yes

# BINDIR is a directory for installing executable programs
BINDIR := /usr/local/bin

ifeq ($(SECCOMP),yes)
    override EXTRA_RUSTFEATURES += seccomp
endif

ifneq ($(EXTRA_RUSTFEATURES),)
    override EXTRA_RUSTFEATURES := --features "$(EXTRA_RUSTFEATURES)"
endif

.DEFAULT_GOAL := default
default: build

build:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE) $(EXTRA_RUSTFEATURES)

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

install:
	install -D $(TARGET_PATH) $(BINDIR)/$(TARGET)

clean:
	cargo clean

vendor:
	cargo vendor

test: test-runk test-agent

test-runk:
	cargo test --all --target $(TRIPLE) $(EXTRA_RUSTFEATURES) -- --nocapture

test-agent:
	make test -C $(AGENT_SOURCE_PATH) STANDARD_OCI_RUNTIME=yes

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
