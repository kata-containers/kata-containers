# Copyright (c) 2019 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0

TARGET := kata-pkgsync

# Define the Go source files by searching in each VPATH element.
SOURCES := $(shell find . -type f 2>&1 | grep -E '.*\.go$$')

ifneq ($(DEBUG),)
  DEBUG_FLAGS := -gcflags=all="-N -l"
endif

VERSION := ${shell cat ./VERSION}
COMMIT_NO := $(shell git rev-parse HEAD 2> /dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no),"${COMMIT_NO}-dirty","${COMMIT_NO}")

$(TARGET): $(SOURCES)
	cd cli && go build $(DEBUG_FLAGS) \
		-ldflags "-X main.name=${TARGET} \
		-X main.commit=${COMMIT} \
		-X main.version=${VERSION}" \
		-o ../$@

.PHONY: all
all: $(TARGET)

.PHONY: clean
clean:
	rm -f $(TARGET)

.PHONY: lint
lint:
	golangci-lint run --enable-all cli/...

