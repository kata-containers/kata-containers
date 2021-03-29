# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# To show variables or targets help on `make help`
# Use the following format:
# '##VAR VARIABLE_NAME: help about variable'
# '##TARGET TARGET_NAME: help about target'

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
COMMIT := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),${COMMIT_NO}-dirty,${COMMIT_NO})
COMMIT_MSG = $(if $(COMMIT),$(COMMIT),unknown)

# Exported to allow cargo to see it
export VERSION_COMMIT := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))

##VAR BUILD_TYPE=release|debug type of rust build
BUILD_TYPE = release

##VAR ARCH=arch target to build (format: uname -m)
ARCH = $(shell uname -m)
##VAR LIBC=musl|gnu
LIBC ?= musl
ifneq ($(LIBC),musl)
    ifeq ($(LIBC),gnu)
        override LIBC = gnu
    else
        $(error "ERROR: A non supported LIBC value was passed. Supported values are musl and gnu")
    endif
endif

ifeq ($(ARCH), ppc64le)
    override ARCH = powerpc64le
    override LIBC = gnu
    $(warning "WARNING: powerpc64le-unknown-linux-musl target is unavailable")
endif

ifeq ($(ARCH), s390x)
    override LIBC = gnu
    $(warning "WARNING: s390x-unknown-linux-musl target is unavailable")
endif


EXTRA_RUSTFLAGS :=
ifeq ($(ARCH), aarch64)
    override EXTRA_RUSTFLAGS = -C link-arg=-lgcc
    $(warning "WARNING: aarch64-musl needs extra symbols from libgcc")
endif

TRIPLE = $(ARCH)-unknown-linux-$(LIBC)

TARGET_PATH = target/$(TRIPLE)/$(BUILD_TYPE)/$(TARGET)

##VAR DESTDIR=<path> is a directory prepended to each installed target file
DESTDIR :=
##VAR BINDIR=<path> is a directory for installing executable programs
BINDIR := /usr/bin

##VAR INIT=yes|no define if agent will be installed as init
INIT := no

# Path to systemd unit directory if installed as not init.
UNIT_DIR := /usr/lib/systemd/system

GENERATED_CODE = src/version.rs

AGENT_NAME=$(TARGET)
API_VERSION=0.0.1
AGENT_VERSION=$(VERSION)

GENERATED_REPLACEMENTS= \
    AGENT_NAME \
    AGENT_VERSION \
    API_VERSION \
    BINDIR \
    COMMIT \
    VERSION_COMMIT
GENERATED_FILES :=

GENERATED_FILES += $(GENERATED_CODE)

ifeq ($(INIT),no)
    # Unit file to start kata agent in systemd systems
    UNIT_FILES = kata-agent.service
    GENERATED_FILES += $(UNIT_FILES)
    # Target to be reached in systemd services
    UNIT_FILES += kata-containers.target
endif

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

define INSTALL_FILE
	install -D -m 644 $1 $(DESTDIR)$2/$1 || exit 1;
endef

##TARGET default: build code
default: $(TARGET) show-header

$(TARGET): $(GENERATED_CODE) $(TARGET_PATH)

$(TARGET_PATH): $(SOURCES) | show-summary
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE)

$(GENERATED_FILES): %: %.in
	@sed $(foreach r,$(GENERATED_REPLACEMENTS),-e 's|@$r@|$($r)|g') "$<" > "$@"

##TARGET optimize: optimized  build
optimize: $(SOURCES) | show-summary show-header
	@RUSTFLAGS="-C link-arg=-s $(EXTRA_RUSTFLAGS) --deny-warnings" cargo build --target $(TRIPLE) --$(BUILD_TYPE)


##TARGET clippy: run clippy linter
clippy: $(GENERATED_CODE)
	cargo clippy --all-targets --all-features --release \
		-- \
		-Aclippy::redundant_allocation \
		-D warnings

format:
	cargo fmt -- --check


##TARGET install: install agent
install: install-services
	@install -D $(TARGET_PATH) $(DESTDIR)/$(BINDIR)/$(TARGET)

##TARGET clean: clean build
clean:
	@cargo clean
	@rm -f $(GENERATED_FILES)
	@rm -f tarpaulin-report.html

#TARGET test: run cargo tests
test:
	@cargo test --all --target $(TRIPLE)

##TARGET check: run test
check: clippy format

##TARGET run: build and run agent
run:
	@cargo run --target $(TRIPLE)

install-services: $(GENERATED_FILES)
ifeq ($(INIT),no)
	@echo "Installing systemd unit files..."
	$(foreach f,$(UNIT_FILES),$(call INSTALL_FILE,$f,$(UNIT_DIR)))
endif

show-header:
	@printf "%s - version %s (commit %s)\n\n" "$(TARGET)" "$(VERSION)" "$(COMMIT_MSG)"

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

## help: Show help comments that start with `##VAR` and `##TARGET`
help: Makefile show-summary
	@echo "==========================Help============================="
	@echo "Variables:"
	@sed -n 's/^##VAR//p' $< | sort
	@echo ""
	@echo "Targets:"
	@sed -n 's/^##TARGET//p' $< | sort

TARPAULIN_ARGS:=-v --workspace
install-tarpaulin:
	cargo install cargo-tarpaulin

# Check if cargo tarpaulin is installed
HAS_TARPAULIN:= $(shell cargo --list | grep tarpaulin 2>/dev/null)
check_tarpaulin:
ifndef  HAS_TARPAULIN
	$(error "tarpaulin is not available please: run make install-tarpaulin ")
else
	$(info OK: tarpaulin installed)
endif

##TARGET codecov: Generate code coverage report
codecov: check_tarpaulin
	cargo tarpaulin $(TARPAULIN_ARGS)

##TARGET codecov-html: Generate code coverage html report
codecov-html: check_tarpaulin
	cargo tarpaulin $(TARPAULIN_ARGS) -o Html

.PHONY: \
	help \
	show-header \
	show-summary \
	optimize

##TARGET generate-protocols: generate/update grpc agent protocols
generate-protocols:
	protocols/hack/update-generated-proto.sh all
