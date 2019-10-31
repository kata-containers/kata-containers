# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

PROJECT_NAME = Kata Containers
PROJECT_URL = https://github.com/kata-containers
PROJECT_COMPONENT = kata-agent

TARGET = $(PROJECT_COMPONENT)

SOURCES := \
  $(shell find . 2>&1 | grep -E '.*\.rs$$') \
  Cargo.toml

VERSION_FILE := ./VERSION
VERSION := $(shell grep -v ^\# $(VERSION_FILE))
COMMIT_NO := $(shell git rev-parse HEAD 2>/dev/null || true)
COMMIT_NO_SHORT := $(shell git rev-parse --short HEAD 2>/dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),${COMMIT_NO}-dirty,${COMMIT_NO})
COMMIT_MSG = $(if $(COMMIT),$(COMMIT),unknown)

# Exported to allow cargo to see it
export VERSION_COMMIT := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))

BUILD_TYPE = release

ARCH = $(shell uname -m)
LIBC = musl
TRIPLE = $(ARCH)-unknown-linux-$(LIBC)

TARGET_PATH = target/$(TRIPLE)/$(BUILD_TYPE)/$(TARGET)

DESTDIR :=
BINDIR := /usr/bin

# Display name of command and it's version (or a message if not available).
#
# Arguments:
#
# 1: Name of command
define get_command_version
$(shell printf "%s: %s\\n" $(1) "$(or $(shell $(1) --version 2>/dev/null), (not available))")
endef

define get_toolchain_version
$(shell printf "%s: %s\\n" "toolchain" "$(or $(shell rustup show active-toolchain 2>/dev/null), (unknown))")
endef

default: $(TARGET) show-header

$(TARGET): $(TARGET_PATH)

$(TARGET_PATH): $(SOURCES) | show-summary
	@cargo build --target $(TRIPLE)

show-header:
	@printf "%s - version %s (commit %s)\n\n" "$(TARGET)" "$(VERSION)" "$(COMMIT_MSG)"

install:
	@install -D $(TARGET_PATH) $(DESTDIR)/$(BINDIR)/$(TARGET)

clean:
	@cargo clean

check:
	@cargo test --target $(TRIPLE)

run:
	@cargo run --target $(TRIPLE)

show-summary: show-header
	@printf "project:\n"
	@printf "  name: $(PROJECT_NAME)\n"
	@printf "  url: $(PROJECT_URL)\n"
	@printf "  component: $(PROJECT_COMPONENT)\n"
	@printf "target: $(TARGET)\n"
	@printf "architecture:\n"
	@printf "  host: $(ARCH)\n"
	@printf "rust:\n"
	@printf "  %s\n" "$(call get_command_version,cargo)"
	@printf "  %s\n" "$(call get_command_version,rustc)"
	@printf "  %s\n" "$(call get_command_version,rustup)"
	@printf "  %s\n" "$(call get_toolchain_version)"
	@printf "\n"

help: show-summary

.PHONY: \
	help \
	show-header \
	show-summary
