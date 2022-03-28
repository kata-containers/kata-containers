# Copyright (c) 2020 Intel Corporation
# Copyright (c) 2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

.DEFAULT_GOAL := default
default: build

build:
	CGO_ENABLED=0 go build -o bin/kata-webhook \
		--ldflags "-w -extldflags '-static'"

image: build
	docker build -t liubin/kata-webhook .

clean:
	rm bin/kata-webhook

test:

install:

vendor:

.PHONY: \
	build \
	check \
	clean \
	install \
	test \
	vendor
