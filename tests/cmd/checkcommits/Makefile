# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

TARGET = checkcommits
SOURCES = $(shell find . 2>&1 | grep -E '.*\.go$$')

VERSION := ${shell cat ./VERSION}
COMMIT_NO := $(shell git rev-parse HEAD 2> /dev/null || true)
COMMIT := $(if $(shell git status --porcelain --untracked-files=no),"${COMMIT_NO}-dirty","${COMMIT_NO}")

default: $(TARGET)

$(TARGET): $(SOURCES)
	go test .
	go install -ldflags "-X main.appCommit=${COMMIT} -X main.appVersion=${VERSION}" .

clean:
	rm -f $(TARGET)
