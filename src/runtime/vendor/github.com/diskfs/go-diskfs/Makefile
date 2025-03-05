.PHONY: test image unit_test

PACKAGE_NAME?=github.com/diskfs/go-diskfs
IMAGE ?= diskfs/go-diskfs:build
GOENV ?= GO111MODULE=on CGO_ENABLED=0
GO_FILES ?= $(shell $(GOENV) go list ./...)
GOBIN ?= $(shell go env GOPATH)/bin
LINTER ?= $(GOBIN)/golangci-lint
LINTER_VERSION ?= v1.61.0

# BUILDARCH is the host architecture
# ARCH is the target architecture
# we need to keep track of them separately
BUILDARCH ?= $(shell uname -m)
BUILDOS ?= $(shell uname -s | tr A-Z a-z)

# canonicalized names for host architecture
ifeq ($(BUILDARCH),aarch64)
BUILDARCH=arm64
endif
ifeq ($(BUILDARCH),x86_64)
BUILDARCH=amd64
endif

# unless otherwise set, I am building for my own architecture, i.e. not cross-compiling
# and for my OS
ARCH ?= $(BUILDARCH)
OS ?= $(BUILDOS)

# canonicalized names for target architecture
ifeq ($(ARCH),aarch64)
        override ARCH=arm64
endif
ifeq ($(ARCH),x86_64)
    override ARCH=amd64
endif

BUILD_CMD = CGO_ENABLED=0 GOOS=$(OS) GOARCH=$(ARCH)
ifdef DOCKERBUILD
BUILD_CMD = docker run --rm \
                -e GOARCH=$(ARCH) \
                -e GOOS=linux \
                -e CGO_ENABLED=0 \
                -v $(CURDIR):/go/src/$(PACKAGE_NAME) \
                -w /go/src/$(PACKAGE_NAME) \
                $(BUILDER_IMAGE)
endif

image:
	docker build -t $(IMAGE) testhelper/docker

# because we keep making the same typo
unit-test: unit_test
unit_test:
	@$(GOENV) go test $(GO_FILES)

test: image
	TEST_IMAGE=$(IMAGE) $(GOENV) go test $(GO_FILES)

golangci-lint: $(LINTER)
$(LINTER):
	curl -sSfL https://raw.githubusercontent.com/golangci/golangci-lint/master/install.sh | sh -s -- -b $(GOBIN) $(LINTER_VERSION)

## Check the file format
fmt-check:
	@if [ -n "$(shell $(BUILD_CMD) gofmt -l ${GO_FILES})" ]; then \
	  $(BUILD_CMD) gofmt -s -e -d ${GO_FILES}; \
	  exit 1; \
	fi

## Lint the files
lint: golangci-lint
	@$(BUILD_CMD) $(LINTER) run ./...

## Vet the files
vet:
	@$(BUILD_CMD) go vet ${GO_FILES}
