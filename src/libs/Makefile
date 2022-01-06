# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

EXTRA_RUSTFEATURES :=

default: build

build:
	cargo build --all-features

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
	cargo test --all $(EXTRA_RUSTFEATURES) -- --nocapture
	@echo "INFO: testing libraries for release build"
	cargo test --release --all $(EXTRA_RUSTFEATURES) -- --nocapture

vendor:
#	cargo vendor

.PHONY: install
