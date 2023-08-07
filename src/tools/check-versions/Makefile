# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

PROJECT_NAME = Kata Containers
PROJECT_URL = https://github.com/kata-containers
PROJECT_COMPONENT = check-versions

TARGET = $(PROJECT_COMPONENT)

VERSION_FILE := ./VERSION
VERSION := $(shell grep -v ^\# $(VERSION_FILE))
COMMIT_NO := $(shell git rev-parse HEAD 2>/dev/null || true)
COMMIT_NO_SHORT := $(shell git rev-parse --short HEAD 2>/dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),${COMMIT_NO}-dirty,${COMMIT_NO})

.DEFAULT_GOAL := default
default: build

build:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) $(if $(findstring release,$(BUILD_TYPE)),--release) $(EXTRA_RUSTFEATURES)

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

clean:
	cargo clean

vendor:
	cargo vendor

test:

install:
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo install --locked --target $(TRIPLE) --path .

check: standard_rust_check

.PHONY: \
	build \
	check \
	clean \
	install \
	test \
	vendor
