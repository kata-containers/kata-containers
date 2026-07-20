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

default: build

# Provides PACKAGE_FLAGS, scoping cargo commands to the packages of
# this component. Note utils.mk defines rules, so it has to be
# included after the default rule.
include ../../utils.mk

build:
	cargo build --all-features

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

check: clippy format

clippy:
	@echo "INFO: cargo clippy..."
	cargo clippy $(PACKAGE_FLAGS) --all-features --release \
		-- \
		-D warnings

format:
	@echo "INFO: cargo fmt..."
	cargo fmt $(PACKAGE_FLAGS) -- --check

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
