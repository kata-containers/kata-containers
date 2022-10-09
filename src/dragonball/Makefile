# Copyright (c) 2019-2022 Alibaba Cloud. All rights reserved.
# Copyright (c) 2019-2022 Ant Group. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

default: build

build:
	cargo build --all-features

check: clippy format

clippy:
	@echo "INFO: cargo clippy..."
	cargo clippy --all-targets --all-features \
		-- \
		-D warnings


vendor: 
	@echo "INFO: vendor do nothing.."

format:
	@echo "INFO: cargo fmt..."
	cargo fmt -- --check

clean:
	cargo clean

test:
	# After we solve the Dragonball UT problem in CI machines (issue: #5378), we will add back the UT test here.
	# @echo "INFO: testing dragonball for development build"
	# cargo test --all-features -- --nocapture
	@echo "Dragonball UT skipped due to issue: #5378"
