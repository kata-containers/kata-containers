#
# Copyright (c) 2017-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

TARGET = kata-github-labels
SOURCES = $(shell find . -type f 2>&1 | grep -E '.*\.go$$')

VERSION := ${shell cat ./VERSION}
COMMIT_NO := $(shell git rev-parse HEAD 2> /dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no),"${COMMIT_NO}-dirty","${COMMIT_NO}")

BINDIR := $(GOPATH)/bin
DESTTARGET := $(abspath $(BINDIR)/$(TARGET))

default: install

check: $(SOURCES)
	go test -v ./...

$(TARGET): $(SOURCES)
	go build -o "$(TARGET)" -ldflags "-X main.name=${TARGET} -X main.commit=${COMMIT} -X main.version=${VERSION}" .

install: $(TARGET)
	install -d $(shell dirname $(DESTTARGET))
	install $(TARGET) $(DESTTARGET)

clean:
	rm -f $(TARGET)

.PHONY: install clean
