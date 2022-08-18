# Copyright (c) 2019-2022 Alibaba Cloud. All rights reserved.
# Copyright (c) 2019-2022 Ant Group. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

default: build

build:
	# FIXME: This line will be removed when we solve the vm-memory dependency problem in Dragonball Sandbox
	cargo update -p vm-memory:0.8.0 --precise 0.7.0
	cargo build --all-features

check: clippy format

clippy:
	@echo "INFO: cargo clippy..."
	cargo clippy --all-targets --all-features \
		-- \
		-D warnings

format:
	@echo "INFO: cargo fmt..."
	cargo fmt -- --check

clean:
	cargo clean

test:
	@echo "INFO: testing dragonball for development build"
	cargo test --all-features -- --nocapture
