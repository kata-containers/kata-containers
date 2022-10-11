# Copyright (c) 2019-2022 Alibaba Cloud. All rights reserved.
# Copyright (c) 2019-2022 Ant Group. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

include ../../utils.mk

ifeq ($(ARCH), s390x)
default build check test clippy:
	@echo "s390x not support currently"
	exit 0
else

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
	@echo "INFO: skip testing dragonball"

endif # ifeq ($(ARCH), s390x)
