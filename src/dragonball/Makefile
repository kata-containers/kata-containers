# Copyright (c) 2019-2022 Alibaba Cloud. All rights reserved.
# Copyright (c) 2019-2022 Ant Group. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

include ../../utils.mk

PROJECT_DIRS := $(shell find . -name Cargo.toml -printf '%h\n' | sort -u)

ifeq ($(ARCH), $(filter $(ARCH), s390x powerpc64le))
default build check test clippy vendor:
	@echo "$(ARCH) is not support currently"
	exit 0
else

default: build

build:
	@echo "INFO: cargo build..."
	cargo build --all-features --target $(TRIPLE)

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

check: clippy format

clippy:
	@echo "INFO: cargo clippy..."
	cargo clippy --all-targets --all-features \
		-- \
		-D warnings

vendor:
	@echo "INFO: vendor do nothing.."

format:
	@echo "INFO: rust fmt..."
	# This is kinda dirty step here simply because cargo fmt --all will apply fmt to all dependencies of dragonball which will include /src/libs/protocols with some file generated during compilation time and could not be formatted when you use cargo fmt --all before building the whole project. In order to avoid this problem, we do fmt check in this following way.
	rustfmt --edition 2018 ./src/dbs_address_space/src/lib.rs ./src/dbs_allocator/src/lib.rs ./src/dbs_arch/src/lib.rs ./src/dbs_boot/src/lib.rs ./src/dbs_device/src/lib.rs ./src/dbs_interrupt/src/lib.rs ./src/dbs_legacy_devices/src/lib.rs ./src/dbs_pci/src/lib.rs ./src/dbs_upcall/src/lib.rs ./src/dbs_utils/src/lib.rs ./src/dbs_virtio_devices/src/lib.rs ./src/lib.rs --check

clean:
	cargo clean

test:
ifdef SUPPORT_VIRTUALIZATION
	@set -e; \
	for dir in $(PROJECT_DIRS); do \
		bash -c "pushd $${dir} && RUST_BACKTRACE=1 cargo test --all-features --target $(TRIPLE) -- --nocapture --test-threads=1 && popd"; \
	done
else
	@echo "INFO: skip testing dragonball, it need virtualization support."
	exit 0
endif

coverage:
	RUST_BACKTRACE=1 cargo llvm-cov --all-features --target $(TRIPLE) -- --nocapture --test-threads=1

endif # ifeq ($(ARCH), s390x)

.DEFAULT_GOAL := default
