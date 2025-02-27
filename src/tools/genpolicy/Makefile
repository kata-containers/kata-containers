# Copyright (c) 2020 Intel Corporation
# Portions Copyright (c) Microsoft Corporation.
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

ifeq ($(ARCH), ppc64le)
     override ARCH = powerpc64le
 endif

COMMIT_HASH := $(shell git rev-parse HEAD 2>/dev/null || true)
# appends '-dirty' to the commit hash if there are uncommitted changes
COMMIT_INFO := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),${COMMIT_HASH}-dirty,${COMMIT_HASH})

GENERATED_CODE = src/version.rs

GENERATED_REPLACEMENTS= COMMIT_INFO
GENERATED_FILES :=

GENERATED_FILES += $(GENERATED_CODE)

$(GENERATED_FILES): %: %.in
	sed $(foreach r,$(GENERATED_REPLACEMENTS),-e 's|@$r@|$($r)|g') "$<" > "$@"

.DEFAULT_GOAL := default
default: build

build: $(GENERATED_FILES)
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE)

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

clean:
	cargo clean
	rm -f $(GENERATED_FILES)

vendor:
	cargo vendor

test: $(GENERATED_FILES)
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo test --all-targets --all-features --target $(TRIPLE)

install: $(GENERATED_FILES)
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo install --locked --target $(TRIPLE) --path .

check:  $(GENERATED_CODE) standard_rust_check

.PHONY: \
	build \
	check \
	clean \
	install \
	test \
	vendor
