# Copyright (c) 2020 Intel Corporation
# Copyright (c) 2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

include ../../../utils.mk

.DEFAULT_GOAL := default
default: build

build:
	CGO_ENABLED=0 go build -o bin/kata-annotation-webhook \
		--ldflags "-w -extldflags '-static'"

image: build
	docker build -t quay.io/kata-containers/kata-annotation-webhook .

clean:
	rm bin/kata-annotation-webhook

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
