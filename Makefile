#
# Copyright 2017 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

all:
	go build proxy.go

test: all
	go test -v

clean:
	rm -f proxy
