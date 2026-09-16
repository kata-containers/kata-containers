# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

IMAGE ?= quay.io/kata-containers/kata-webhook-example:latest
GO_VERSION ?= $(shell yq '.languages.golang.version' $(CURDIR)/../../../versions.yaml)

all: docker

docker:
	docker build \
	       -f $(CURDIR)/Dockerfile \
	       --build-arg GO_VERSION=$(GO_VERSION) \
	       -t $(IMAGE) \
	       $(CURDIR)

.PHONY: all docker
