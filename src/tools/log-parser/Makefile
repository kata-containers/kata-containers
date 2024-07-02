#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

TARGET = kata-log-parser
SOURCES = $(shell find . 2>&1 | grep -E '.*\.go$$')

VERSION_FILE := ./VERSION
VERSION := $(shell grep -v ^\# $(VERSION_FILE) 2>/dev/null || echo "unknown")
COMMIT_NO := $(shell git rev-parse HEAD 2>/dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no 2>/dev/null || true),"${COMMIT_NO}-dirty","${COMMIT_NO}")

BINDIR := $(GOPATH)/bin
DESTTARGET := $(abspath $(BINDIR)/$(TARGET))

default: install

check:
	go test .

test:
	go test -v .

$(TARGET): $(SOURCES) check
	go build -o "$(TARGET)" -ldflags "-X main.name=${TARGET} -X main.commit=${COMMIT} -X main.version=${VERSION}" .

install: $(TARGET)
	install -d $(shell dirname $(DESTTARGET))
	install $(TARGET) $(DESTTARGET)

static-checks-build:
	@echo "INFO: static-checks-build do nothing.."

clean:
	rm -f $(TARGET)

.PHONY: test install clean
