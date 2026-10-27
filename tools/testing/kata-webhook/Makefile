# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

IMAGE ?= quay.io/kata-containers/kata-webhook-example:latest
GO_VERSION ?= $(shell yq '.languages.golang.version' $(CURDIR)/../../../versions.yaml)
# Conformance CI builds with podman. A local `make docker` keeps using docker.
BUILDER ?= docker
BUILD_FLAGS ?=

all: docker

docker:
	$(BUILDER) build \
	       $(BUILD_FLAGS) \
	       -f $(CURDIR)/Dockerfile \
	       --build-arg GO_VERSION=$(GO_VERSION) \
	       -t $(IMAGE) \
	       $(CURDIR)

.PHONY: all docker
