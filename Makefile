#
# Copyright 2017 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

TARGET = kata-proxy

all: $(TARGET)

$(TARGET):
	go build -o $@ proxy.go

test:
	go test -v -race -coverprofile=coverage.txt -covermode=atomic

clean:
	rm -f $(TARGET)
