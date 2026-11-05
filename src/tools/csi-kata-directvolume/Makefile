#
# Copyright 2019 The Kubernetes Authors.
# Copyright (c) 2023 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

CMDS=directvolplugin
all: build

include release-tools/build.make

# Static checks: build everything and vet it, mirroring what CI's
# "make check" expects from a component.
.PHONY: check
check:
	go build ./...
	go vet ./...
