# Copyright (c) 2026 Florian Vichot
# SPDX-License-Identifier: Apache-2.0

all: docker

docker:
	docker build \
	       -f Dockerfile \
	       --ignorefile dockerignore \
	       --build-arg GO_VERSION=$$(yq '.languages.golang.version' $(CURDIR)/../../../versions.yaml) \
	       -t kata-monitor:$$(cat $(CURDIR)/../../../VERSION) \
	       $(CURDIR)/../../..
