# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

EXTRA_RUSTFEATURES :=

EXTRA_TEST_FLAGS :=
USERID=$(shell id -u)
ifeq ($(USERID), 0)
    override EXTRA_TEST_FLAGS = --ignored
endif

##VAR PULL_TYPE=default|host-share-image-block define if agent enables the host share image blocks with dm-verity feature
PULL_TYPE ?= default

# Enable pull-type feature of rust build
ifeq ($(PULL_TYPE),host-share-image-block)
    override EXTRA_RUSTFEATURES += dm-verity
endif


default: build

build:
	cargo build --all-features

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

check: clippy format

clippy:
	@echo "INFO: cargo clippy..."
	cargo clippy --all-targets --all-features --release \
		-- \
		-D warnings

format:
	@echo "INFO: cargo fmt..."
	cargo fmt -- --check

clean:
	cargo clean

# It is essential to run these tests using *both* build profiles.
# See the `test_logger_levels()` test for further information.
test:
	@echo "INFO: testing libraries for development build"
	cargo test --all $(EXTRA_RUSTFEATURES) -- --nocapture $(EXTRA_TEST_FLAGS)
	@echo "INFO: testing libraries for release build"
	cargo test --release --all $(EXTRA_RUSTFEATURES) -- --nocapture $(EXTRA_TEST_FLAGS)

.PHONY: install vendor
