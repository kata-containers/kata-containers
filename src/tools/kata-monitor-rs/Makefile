# Copyright (c) 2026 Ant Group
#
# SPDX-License-Identifier: Apache-2.0

PROJECT_NAME = Kata Containers
PROJECT_URL = https://github.com/kata-containers
PROJECT_COMPONENT = kata-monitor-rs

TARGET = $(PROJECT_COMPONENT)

.DEFAULT_GOAL := default

default: build

build:
	@cargo build --release

clean:
	@cargo clean

test:
	@cargo test

install:
	@cargo install --path .

check:
	@cargo clippy -- -D warnings

.PHONY: \
	build \
	check \
	clean \
	default \
	install \
	test
