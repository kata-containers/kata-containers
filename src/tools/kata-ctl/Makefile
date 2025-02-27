# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

PROJECT_NAME = Kata Containers
PROJECT_URL = https://github.com/kata-containers
PROJECT_COMPONENT = kata-ctl

TARGET = $(PROJECT_COMPONENT)
INSTALL_PATH = $(HOME)/.cargo

VERSION_FILE := ./VERSION
export VERSION := $(shell grep -v ^\# $(VERSION_FILE) 2>/dev/null || echo "unknown")
COMMIT_NO := $(shell git rev-parse HEAD 2>/dev/null || true)
COMMIT_NO_SHORT := $(shell git rev-parse --short HEAD 2>/dev/null || true)
export COMMIT := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),${COMMIT_NO}-dirty,${COMMIT_NO})

# Exported to allow cargo to see it
export KATA_CTL_VERSION := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))

GENERATED_CODE = src/ops/version.rs

GENERATED_REPLACEMENTS= \
    KATA_CTL_VERSION \
    VERSION \
    COMMIT

GENERATED_FILES := $(GENERATED_CODE)

.DEFAULT_GOAL := default

ifeq ($(ARCH), ppc64le)
default build check test vendor:
	@echo "Building kata-ctl on $(ARCH) is currently being skipped"
	exit 0
else

default: $(TARGET) build

$(TARGET): $(GENERATED_CODE)

build:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) $(if $(findstring release,$(BUILD_TYPE)),--release) $(EXTRA_RUSTFEATURES)

static-checks-build: $(GENERATED_CODE)

$(GENERATED_FILES): %: %.in
	@sed $(foreach r,$(GENERATED_REPLACEMENTS),-e 's|@$r@|$($r)|g') "$<" > "$@"


clean:
	@cargo clean
	@rm -f $(GENERATED_FILES)

vendor:
	cargo vendor

test: $(GENERATED_CODE)
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo test --target $(TRIPLE) $(if $(findstring release,$(BUILD_TYPE)),--release) $(EXTRA_RUSTFEATURES) -- --nocapture

install:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo install --locked --target $(TRIPLE) --path . --root $(INSTALL_PATH)

check: $(GENERATED_CODE) standard_rust_check

endif

.PHONY: \
	build \
	check \
	clean \
	install \
	test \
	vendor
