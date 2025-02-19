# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Note:
#
# Since this file defines rules, it should be included
# in other makefiles *after* their default rule has been defined.

# Owner for installed files
export KATA_INSTALL_OWNER     ?= root

# Group for installed files
export KATA_INSTALL_GROUP     ?= adm

# Permissions for installed configuration files.
#
# XXX: Note that the permissions MUST be zero for "other"
# XXX: in case the configuration file contains secrets.
export KATA_INSTALL_CFG_PERMS ?= 0640

# Create a set of standard rules for a project such that:
#
# - The component depends on its Makefile.
# - "build-$(component)" is an alias for "$(component)".
#
# Parameters:
#
# $1 - Directory component lives in.
# $2 - Name of component.
#
# Note: The "clean" and "vendor" rules are the "odd one out" - they only
# depend on the Makefile. This ensure that running them won't first try
# to build the project.

define make_rules
$(2) : $(1)/$(2)/Makefile
	make -C $(1)/$(2)
build-$(2) : $(2)

static-checks-build-$(2):
	make -C $(1)/$(2) static-checks-build

check-$(2) : $(2)
	make -C $(1)/$(2) check

vendor-$(2) : $(1)/$(2)/Makefile
	make -C $(1)/$(2) vendor

clean-$(2) : $(1)/$(2)/Makefile
	make -C $(1)/$(2) clean

install-$(2) : $(2)
	make -C $(1)/$(2) install

test-$(2) : $(2)
	make -C $(1)/$(2) test

.PHONY: \
    $(2) \
    build-$(2) \
    clean-$(2) \
    check-$(2) \
    vendor-$(2) \
    test-$(2) \
    install-$(2)
endef

# Define a set of rules for a source component.
#
# Parameters:
#
# $1 - Name of component.

define make_component_rules
$(eval $(call make_rules,src,$(1)))
endef

# Define a set of rules for a tool.
#
# Parameters:
#
# $1 - name of tool

define make_tool_rules
$(eval $(call make_rules,src/tools,$(1)))
endef

# Create a "${target}-all" alias which will cause each component/tool
# rule to be called.
#
# Parameters:
#
# $1 - List of targets to create rules for.

define make_all_rules
$(1)-all: $(foreach c,$(COMPONENTS) $(TOOLS),$(1)-$(c))

.PHONY: $(1) $(1)-all
endef

# Create all rules for the caller.
#
# Entry point to this file.
#
# Parameters:
#
# $1 - List of components.
# $2 - List of tools.
# $3 - List of standard targets.
define create_all_rules

all: $(1) $(2)

# Create rules for all components.
$(foreach c,$(1),$(eval $(call make_component_rules,$(c))))

# Create rules for all tools.
$(foreach c,$(2),$(eval $(call make_tool_rules,$(c))))

# Create the "-all" rules.
$(foreach a,$(3),$(eval $(call make_all_rules,$(a))))

# Support "make ${target}"
# (which is an alias for "make ${target}-all").
$(3) : % : %-all

endef


##VAR BUILD_TYPE=release|debug type of rust build
BUILD_TYPE ?= release

##VAR ARCH=arch target to build (format: uname -m)
HOST_ARCH = $(shell uname -m)
ARCH ?= $(HOST_ARCH)
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
    override LIBC = gnu
    override ARCH = powerpc64le
    $(warning "WARNING: powerpc64le-unknown-linux-musl target is unavailable")
endif

ifeq ($(ARCH), s390x)
    override LIBC = gnu
    $(warning "WARNING: s390x-unknown-linux-musl target is unavailable")
endif


EXTRA_RUSTFLAGS :=

ifneq ($(HOST_ARCH),$(ARCH))
    ifeq ($(CC),)
         CC = gcc
         $(warning "WARNING: A foreign ARCH was passed, but no CC alternative. Using gcc.")
    endif
    override EXTRA_RUSTFLAGS += -C linker=$(CC)
    undefine CC
endif

TRIPLE = $(ARCH)-unknown-linux-$(LIBC)

CWD := $(shell dirname $(realpath $(firstword $(MAKEFILE_LIST))))

standard_rust_check:
	@echo "standard rust check..."
	cargo fmt -- --check
	cargo clippy --all-targets --all-features --release \
		-- \
		-D warnings

# Install a file (full version).
#
# params:
#
# $1 : File to install.
# $2 : Directory path where file will be installed.
# $3 : Permissions to apply to the installed file.
define INSTALL_FILE_FULL
    sudo install \
        --mode $3 \
        --owner $(KATA_INSTALL_OWNER) \
        --group $(KATA_INSTALL_GROUP) \
        -D $1 $2/$(notdir $1) || exit 1;
endef

# Install a configuration file.
#
# params:
#
# $1 : File to install.
# $2 : Directory path where file will be installed.
define INSTALL_CONFIG
    $(call INSTALL_FILE_FULL,$1,$2,$(KATA_INSTALL_CFG_PERMS))
endef
