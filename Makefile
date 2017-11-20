#
# Copyright 2017 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

all:
	go build proxy.go
	make -C test

test: all
	make -C test test

clean:
	rm -f proxy
	make -C test clean
