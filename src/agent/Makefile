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

VERSION_FILE := ./VERSION
VERSION := $(shell grep -v ^\# $(VERSION_FILE) 2>/dev/null || echo "unknown")
COMMIT_NO := $(shell git rev-parse HEAD 2>/dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),${COMMIT_NO}-dirty,${COMMIT_NO})
COMMIT_MSG = $(if $(COMMIT),$(COMMIT),unknown)

# Exported to allow cargo to see it
export VERSION_COMMIT := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))

EXTRA_RUSTFEATURES :=

##VAR SECCOMP=yes|no define if agent enables seccomp feature
SECCOMP ?= yes

# Enable seccomp feature of rust build
ifeq ($(SECCOMP),yes)
    override EXTRA_RUSTFEATURES += seccomp
endif

##VAR AGENT_POLICY=yes|no define if agent enables the policy feature
AGENT_POLICY ?= no

# Enable the policy feature of rust build
ifeq ($(AGENT_POLICY),yes)
    override EXTRA_RUSTFEATURES += agent-policy
endif

##VAR PULL_TYPE=default|guest-pull define if agent enables the guest pull image feature
PULL_TYPE ?= default
ifeq ($(PULL_TYPE),default)
    override EXTRA_RUSTFEATURES += default-pull
# Enable guest pull image feature of rust build
else ifeq ($(PULL_TYPE),guest-pull)
    override EXTRA_RUSTFEATURES += guest-pull
endif


include ../../utils.mk

ifeq ($(ARCH), ppc64le)
    override ARCH = powerpc64le
endif

##VAR STANDARD_OCI_RUNTIME=yes|no define if agent enables standard oci runtime feature
STANDARD_OCI_RUNTIME := no

# Enable standard oci runtime feature of rust build
ifeq ($(STANDARD_OCI_RUNTIME),yes)
    override EXTRA_RUSTFEATURES += standard-oci-runtime
endif

ifneq ($(EXTRA_RUSTFEATURES),)
    override EXTRA_RUSTFEATURES := --features "$(EXTRA_RUSTFEATURES)"
endif

TARGET_PATH = target/$(TRIPLE)/$(BUILD_TYPE)/$(TARGET)

##VAR DESTDIR=<path> is a directory prepended to each installed target file
DESTDIR ?=
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

.DEFAULT_GOAL := default

##TARGET default: build code
default: $(TARGET) show-header

static-checks-build: $(GENERATED_CODE)

$(TARGET): $(GENERATED_CODE) $(TARGET_PATH)

$(TARGET_PATH): show-summary
	@RUSTFLAGS="$(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) $(if $(findstring release,$(BUILD_TYPE)),--release) $(EXTRA_RUSTFEATURES)

$(GENERATED_FILES): %: %.in
	@sed $(foreach r,$(GENERATED_REPLACEMENTS),-e 's|@$r@|$($r)|g') "$<" > "$@"

##TARGET optimize: optimized  build
optimize: show-summary show-header
	@RUSTFLAGS="-C link-arg=-s $(EXTRA_RUSTFLAGS) --deny warnings" cargo build --target $(TRIPLE) $(if $(findstring release,$(BUILD_TYPE)),--release) $(EXTRA_RUSTFEATURES)

##TARGET install: install agent
install: install-services
	@install -D $(TARGET_PATH) $(DESTDIR)/$(BINDIR)/$(TARGET)

##TARGET clean: clean build
clean:
	@cargo clean
	@rm -f $(GENERATED_FILES)
	@rm -f tarpaulin-report.html

vendor:
	@cargo vendor


#TARGET test: run cargo tests
test: $(GENERATED_FILES)
	@RUST_LIB_BACKTRACE=0 RUST_BACKTRACE=1 cargo test --all --target $(TRIPLE) $(EXTRA_RUSTFEATURES) -- --nocapture

##TARGET check: run test
check: $(GENERATED_FILES) standard_rust_check

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
	optimize \
	show-header \
	show-summary \
	vendor

##TARGET generate-protocols: generate/update grpc agent protocols
generate-protocols:
	../libs/protocols/hack/update-generated-proto.sh all
