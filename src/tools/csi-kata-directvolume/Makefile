#
# Copyright 2019 The Kubernetes Authors.
# Copyright (c) 2023 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

CMDS=directvolplugin
all: build

include release-tools/build.make

# Static checks for CI's "make check" matrix entry, keeping this component at
# parity with the other Go components in the stack.
#
# Where each check comes from:
#   - "make build" and "make test" come from the included
#     release-tools/build.make. Despite its name, build.make's test-vet
#     recipe runs "go test ./..." (it does not run go vet), and its
#     test-fmt recipe runs gofmt.
#   - "make check" below adds golangci-lint, like src/runtime's lint
#     target. The pinned version is versions.yaml's
#     languages.golangci-lint.version, and the config is the repo-wide
#     tests/.golangci.yml (which enables the govet linter, so bare
#     "go vet" is subsumed by this run).
.PHONY: check
check:
	go build ./...
	@GOLANGCI_LINT=$$(command -v golangci-lint); \
	if [ -z "$$GOLANGCI_LINT" ]; then \
		echo "ERROR: golangci-lint not found. Its version is pinned in versions.yaml (languages.golangci-lint.version); tests/static-checks.sh auto-installs it in CI. Install it in \$$(go env GOPATH)/bin first." >&2; \
		exit 1; \
	fi; \
	CONFIG_PATH=$$(cd ../../.. && pwd)/tests/.golangci.yml; \
	if [ -f "$$CONFIG_PATH" ]; then \
		$$GOLANGCI_LINT run -c "$$CONFIG_PATH"; \
	else \
		echo "WARNING: $$CONFIG_PATH not found, running golangci-lint with defaults"; \
		$$GOLANGCI_LINT run; \
	fi
