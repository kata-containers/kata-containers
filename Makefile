#
# Copyright 2017 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

DESTDIR :=
ifeq ($(PREFIX),)
PREFIX := /usr
endif
LIBEXECDIR := $(PREFIX)/libexec
PROJECT := kata-containers
# Override will ignore PREFIX, LIBEXECDIR and PROJECT
INSTALLDIR := $(LIBEXECDIR)/$(PROJECT)

TARGET = kata-proxy
SOURCES := $(shell find . 2>&1 | grep -E '.*\.go$$')

VERSION_FILE := ./VERSION
VERSION := $(shell grep -v ^\# $(VERSION_FILE))
COMMIT_NO := $(shell git rev-parse HEAD 2> /dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no),${COMMIT_NO}-dirty,${COMMIT_NO})
VERSION_COMMIT := $(if $(COMMIT),$(VERSION)-$(COMMIT),$(VERSION))
# go build common flags
BUILDFLAGS := -buildmode=pie

$(TARGET): $(SOURCES) $(VERSION_FILE)
	go build $(BUILDFLAGS) -o $@ -ldflags "-X main.version=$(VERSION_COMMIT)"

test:
	bash .ci/go-test.sh

clean:
	rm -f $(TARGET)

install:
	install -D $(TARGET) $(DESTDIR)/$(INSTALLDIR)/$(TARGET)

check: check-go-static

check-go-static:
	bash .ci/static-checks.sh
