#
# Copyright 2017 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

# Set to "yes“ if binary stripping is needed.
STRIP := no

DESTDIR :=
ifeq ($(PREFIX),)
PREFIX := /usr
endif
LIBEXECDIR := $(PREFIX)/libexec
PROJECT := kata-containers
# Override will ignore PREFIX, LIBEXECDIR and PROJECT
INSTALLDIR := $(LIBEXECDIR)/$(PROJECT)

TARGET = kata-shim
SOURCES := $(shell find . 2>&1 | grep -E '.*\.go$$')

VERSION_FILE := ./VERSION
VERSION := $(shell grep -v ^\# $(VERSION_FILE))
COMMIT_NO := $(shell git rev-parse HEAD 2> /dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no),${COMMIT_NO}-dirty,${COMMIT_NO})
VERSION_COMMIT := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))
# go build common flags
BUILDFLAGS := -buildmode=pie

# whether stipping the binary
ifeq ($(STRIP),yes)
       KATA_LDFLAGS += -w -s
endif

$(TARGET): $(SOURCES) $(VERSION_FILE)
	go build $(BUILDFLAGS) -o $@ -ldflags "-X main.version=$(VERSION_COMMIT) $(KATA_LDFLAGS)"

test:
	@echo "Go tests using faketty"
	bash .ci/faketty.sh .ci/go-test.sh
	#Run again without fake tty to avoid hide any potential issue.
	@echo "Go tests without faketty"
	bash .ci/go-test.sh

clean:
	rm -f $(TARGET)

install:
	install -D $(TARGET) $(DESTDIR)/$(INSTALLDIR)/$(TARGET)

check: check-go-static

check-go-static:
	bash .ci/static-checks.sh
