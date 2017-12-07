#
# Copyright 2017 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

TARGET = kata-proxy
SOURCES := $(shell find . 2>&1 | grep -E '.*\.go$$')

$(TARGET): $(SOURCES)
	go build -o $@ proxy.go

test:
	go test -v -race -coverprofile=coverage.txt -covermode=atomic

clean:
	rm -f $(TARGET)
