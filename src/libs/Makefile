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

# Find all packages of this component
PACKAGES := $(shell cargo metadata --no-deps --format-version 1 | \
                jq -r --arg d "$(CURDIR)/" '.packages[] | select(.manifest_path | startswith($$d)) | .name')
PACKAGE_FLAGS := $(patsubst %,-p %,$(PACKAGES))

default: build

build:
	cargo build --all-features

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

check: clippy format

clippy:
	@echo "INFO: cargo clippy..."
	cargo clippy $(LIBS) --all-features --release \
		-- \
		-D warnings

format:
	@echo "INFO: cargo fmt..."
	cargo fmt $(LIBS) -- --check

clean:
	cargo clean

# It is essential to run these tests using *both* build profiles.
# See the `test_logger_levels()` test for further information.
test:
	@echo "INFO: testing libraries for development build"
	cargo test $(PACKAGE_FLAGS) $(EXTRA_RUSTFEATURES) -- --nocapture $(EXTRA_TEST_FLAGS)
	@echo "INFO: testing libraries for release build"
	cargo test --release $(PACKAGE_FLAGS) $(EXTRA_RUSTFEATURES) -- --nocapture $(EXTRA_TEST_FLAGS)

.PHONY: install
