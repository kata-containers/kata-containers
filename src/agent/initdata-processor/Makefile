# Copyright (c) 2026 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

PROJECT_COMPONENT = initdata-processor

TARGET = $(PROJECT_COMPONENT)

.PHONY: all build test clean install uninstall check fmt clippy

##VAR PREFIX=installation prefix (default: /usr)
PREFIX ?= /usr
##VAR BINDIR=directory for installing executable programs (default: $(PREFIX)/bin)
BINDIR ?= $(PREFIX)/bin
##VAR SYSTEMD_DIR=directory for systemd unit files (default: /usr/lib/systemd/system)
SYSTEMD_DIR ?= /usr/lib/systemd/system
##VAR DESTDIR=directory prepended to each installed target file
DESTDIR ?=

.DEFAULT_GOAL := default

##TARGET default: build code
default: build

##TARGET build: build the initdata-processor binary
build:
	@echo "Building $(TARGET)..."
	cargo build --release

##TARGET test: run cargo tests
test:
	@echo "Running tests..."
	RUST_LIB_BACKTRACE=0 RUST_BACKTRACE=1 cargo test -- --nocapture

##TARGET check: run code quality checks (fmt + clippy)
check: fmt clippy

##TARGET fmt: check code formatting
fmt:
	@echo "Checking code formatting..."
	cargo fmt -- --check

##TARGET clippy: run clippy linter
clippy:
	@echo "Running clippy..."
	cargo clippy --all-targets --all-features --release --locked \
		-- \
		-D warnings

##TARGET clean: clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

##TARGET install: install binary and systemd service
install: build
	@echo "Installing $(TARGET)..."
	@install -D -m 755 ./target/release/$(TARGET) $(DESTDIR)$(BINDIR)/$(TARGET)
	@install -D -m 644 systemd-services/initdata-processor.service $(DESTDIR)$(SYSTEMD_DIR)/initdata-processor.service
	@echo "Installation complete."
	@echo "Binary installed to: $(DESTDIR)$(BINDIR)/$(TARGET)"
	@echo "Systemd service installed to: $(DESTDIR)$(SYSTEMD_DIR)/initdata-processor.service"

##TARGET uninstall: remove installed files
uninstall:
	@echo "Uninstalling $(TARGET)..."
	@rm -f $(DESTDIR)$(BINDIR)/$(TARGET)
	@rm -f $(DESTDIR)$(SYSTEMD_DIR)/initdata-processor.service
	@echo "Uninstallation complete."