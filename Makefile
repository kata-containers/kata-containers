#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# The time limit in seconds for each test
TIMEOUT ?= 60

default: checkcommits

checkcommits:
	make -C cmd/checkcommits

ginkgo:
	ln -sf . vendor/src
	GOPATH=$(PWD)/vendor go build ./vendor/github.com/onsi/ginkgo/ginkgo
	unlink vendor/src

functional: ginkgo
ifeq (${RUNTIME},)
	$(error RUNTIME is not set)
else
	./ginkgo -v functional/ -- -runtime=${RUNTIME} -timeout=${TIMEOUT}
endif

integration: ginkgo
ifeq ($(RUNTIME),)
	$(error RUNTIME is not set)
else
	./ginkgo -v -focus "${FOCUS}" ./integration/docker/ -- -runtime=${RUNTIME} -timeout=${TIMEOUT}
endif

crio:
	bash .ci/install_bats.sh
	RUNTIME=${RUNTIME} ./integration/cri-o/cri-o.sh

log-parser:
	make -C cmd/log-parser

test: functional integration crio

check: checkcommits log-parser

.PHONY: check checkcommits integration ginkgo log-parser test crio
