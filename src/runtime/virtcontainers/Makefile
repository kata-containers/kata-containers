#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

PREFIX := /usr
HOOK_DIR := hook/mock
HOOK_BIN := hook
MK_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
GOBUILD_FLAGS := -mod=vendor

#
# Pretty printing
#

V	      = @
Q	      = $(V:1=)
QUIET_GOBUILD = $(Q:@=@echo    '     GOBUILD  '$@;)

#
# Build
#

all: build binaries

build:
	$(QUIET_GOBUILD)go build $(GOBUILD_FLAGS) $(go list ./... | grep -v /vendor/)

hook:
	$(QUIET_GOBUILD)go build $(GOBUILD_FLAGS) -o $(HOOK_DIR)/$@ $(HOOK_DIR)/*.go

binaries: hook

#
# Tests
#

check: check-go-static check-go-test

check-go-static:
	bash $(MK_DIR)/../../../ci/static-checks.sh

check-go-test:
	bash $(MK_DIR)/../../../ci/go-test.sh

#
# Clean
#

# Input: filename to check.
# Output: filename, assuming the file exists and is safe to delete.
define FILE_SAFE_TO_REMOVE =
$(shell test -e "$(1)" && test "$(1)" != "/" && echo "$(1)")
endef

CLEAN_FILES += $(HOOK_DIR)/$(HOOK_BIN)

clean:
	rm -f $(foreach f,$(CLEAN_FILES),$(call FILE_SAFE_TO_REMOVE,$(f)))

.PHONY: \
	all \
	build \
	hook \
	binaries \
	check \
	check-go-static \
	check-go-test \
	clean
