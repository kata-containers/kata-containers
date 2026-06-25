# Copyright (c) 2020-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# List of available components
COMPONENTS =

COMPONENTS += libs
COMPONENTS += agent
COMPONENTS += dragonball
COMPONENTS += runtime
COMPONENTS += runtime-rs

# List of available tools
TOOLS =

TOOLS += agent-ctl
TOOLS += kata-ctl
TOOLS += log-parser
TOOLS += trace-forwarder

STANDARD_TARGETS = build check clean install static-checks-build test

# Variables for the build-and-publish-kata-debug target
KATA_DEBUG_REGISTRY ?= ""
KATA_DEBUG_TAG ?= ""

default: all

include utils.mk
include ./tools/packaging/kata-deploy/local-build/Makefile

# Create the rules
$(eval $(call create_all_rules,$(COMPONENTS),$(TOOLS),$(STANDARD_TARGETS)))

# Non-standard rules

generate-protocols:
	make -C src/agent generate-protocols

# Some static checks rely on generated source files of components.
static-checks: static-checks-build
	bash tests/static-checks.sh

build-and-publish-kata-debug:
	bash tools/packaging/kata-debug/kata-debug-build-and-upload-payload.sh ${KATA_DEBUG_REGISTRY} ${KATA_DEBUG_TAG}

docs-build:
	docker build -t kata-docs:latest -f ./docs/Dockerfile ./docs

docs-serve: docs-build
	docker run --rm -p 8000:8000 -v ${PWD}:/docs:ro kata-docs:latest serve --config-file /docs/mkdocs.yaml -a 0.0.0.0:8000

CSPELL_IMAGE ?= ghcr.io/streetsidesoftware/cspell@sha256:f02e91044d7ab4c31aab76e9b87943a1c8a229f30ce684ca1f04f941084cb049
docs-spellcheck:
	docker run --rm -v ${PWD}:/workdir:ro -w /workdir ${CSPELL_IMAGE} --config .cspell.yaml "**/*.md" "**/*.rst" "**/*.txt"

docs-editorconfig-checker:
	docker run --rm --volume=${PWD}:/check mstruebing/editorconfig-checker:v3.7

docs-lint: docs-spellcheck docs-editorconfig-checker

.PHONY: \
	all \
	kata-tarball \
	install-tarball \
	default \
	static-checks \
	docs-build \
	docs-serve \
	docs-spellcheck \
	docs-editorconfig-checker \
	docs-lint
