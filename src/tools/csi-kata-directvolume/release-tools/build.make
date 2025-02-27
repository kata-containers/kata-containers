# Copyright 2019 The Kubernetes Authors.
#
# SPDX-License-Identifier: Apache-2.0
#

.PHONY: build-% build container-% container push-% push clean test

# A space-separated list of all commands in the repository, must be
# set in main Makefile of a repository.
# CMDS=

# This is the default. It can be overridden in the main Makefile after
# including build.make.
REGISTRY_NAME=quay.io/k8scsi

# Can be set to -mod=vendor to ensure that the "vendor" directory is used.
GOFLAGS_VENDOR=

# Revision that gets built into each binary via the main.version
# string. Uses the `git describe` output based on the most recent
# version tag with a short revision suffix or, if nothing has been
# tagged yet, just the revision.
#
# Beware that tags may also be missing in shallow clones as done by
# some CI systems (like TravisCI, which pulls only 50 commits).
REV=$(shell git describe --long --tags --match='v*' --dirty 2>/dev/null || git rev-list -n1 HEAD)

# A space-separated list of image tags under which the current build is to be pushed.
# Determined dynamically.
IMAGE_TAGS=

# A "canary" image gets built if the current commit is the head of the remote "master" branch.
# That branch does not exist when building some other branch in TravisCI.
IMAGE_TAGS+=$(shell if [ "$$(git rev-list -n1 HEAD)" = "$$(git rev-list -n1 origin/master 2>/dev/null)" ]; then echo "canary"; fi)

# A "X.Y.Z-canary" image gets built if the current commit is the head of a "origin/release-X.Y.Z" branch.
# The actual suffix does not matter, only the "release-" prefix is checked.
IMAGE_TAGS+=$(shell git branch -r --points-at=HEAD | grep 'origin/release-' | grep -v -e ' -> ' | sed -e 's;.*/release-\(.*\);\1-canary;')

# A release image "vX.Y.Z" gets built if there is a tag of that format for the current commit.
# --abbrev=0 suppresses long format, only showing the closest tag.
IMAGE_TAGS+=$(shell tagged="$$(git describe --tags --match='v*' --abbrev=0)"; if [ "$$tagged" ] && [ "$$(git rev-list -n1 HEAD)" = "$$(git rev-list -n1 $$tagged)" ]; then echo $$tagged; fi)

# Images are named after the command contained in them.
IMAGE_NAME=$(REGISTRY_NAME)/$*

ifdef V
# Adding "-alsologtostderr" assumes that all test binaries contain glog. This is not guaranteed.
TESTARGS = -v -args -alsologtostderr -v 5
else
TESTARGS =
endif

ARCH := $(if $(GOARCH),$(GOARCH),$(shell go env GOARCH))

# Specific packages can be excluded from each of the tests below by setting the *_FILTER_CMD variables
# to something like "| grep -v 'github.com/kubernetes-csi/project/pkg/foobar'". See usage below.

build-%:
	mkdir -p bin
	CGO_ENABLED=0 GOOS=linux go build $(GOFLAGS_VENDOR) -a -ldflags '-X main.version=$(REV) -extldflags "-static"' -o ./bin/$* ./cmd/$*
	if [ "$$ARCH" = "amd64" ]; then \
		CGO_ENABLED=0 GOOS=windows go build $(GOFLAGS_VENDOR) -a -ldflags '-X main.version=$(REV) -extldflags "-static"' -o ./bin/$*.exe ./cmd/$* ; \
		CGO_ENABLED=0 GOOS=linux GOARCH=ppc64le go build $(GOFLAGS_VENDOR) -a -ldflags '-X main.version=$(REV) -extldflags "-static"' -o ./bin/$*-ppc64le ./cmd/$* ; \
	fi

container-%: build-%
	docker build -t $*:latest -f $(shell if [ -e ./cmd/$*/Dockerfile ]; then echo ./cmd/$*/Dockerfile; else echo Dockerfile; fi) --label revision=$(REV) .

push-%: container-%
	set -ex; \
	push_image () { \
		docker tag $*:latest $(IMAGE_NAME):$$tag; \
		docker push $(IMAGE_NAME):$$tag; \
	}; \
	for tag in $(IMAGE_TAGS); do \
		if [ "$$tag" = "canary" ] || echo "$$tag" | grep -q -e '-canary$$'; then \
			: "creating or overwriting canary image"; \
			push_image; \
		elif docker pull $(IMAGE_NAME):$$tag 2>&1 | tee /dev/stderr | grep -q "manifest for $(IMAGE_NAME):$$tag not found"; then \
			: "creating release image"; \
			push_image; \
		else \
			: "release image $(IMAGE_NAME):$$tag already exists, skipping push"; \
		fi; \
	done

build: $(CMDS:%=build-%)
container: $(CMDS:%=container-%)
push: $(CMDS:%=push-%)

clean:
	-rm -rf bin

.PHONY: test-vet
test: test-vet
test-vet:
	@ echo; echo "### $@:"
	go test $(GOFLAGS_VENDOR) `go list $(GOFLAGS_VENDOR) ./... | grep -v vendor $(TEST_VET_FILTER_CMD)`

.PHONY: test-fmt
test: test-fmt
test-fmt:
	@ echo; echo "### $@:"
	files=$$(find . -name '*.go' | grep -v './vendor' $(TEST_FMT_FILTER_CMD)); \
	if [ $$(gofmt -d $$files | wc -l) -ne 0 ]; then \
		echo "formatting errors:"; \
		gofmt -d $$files; \
		false; \
	fi
