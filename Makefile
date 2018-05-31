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

docker-compose:
	bash .ci/install_bats.sh
	cd integration/docker-compose && \
	bats docker-compose.bats

kubernetes:
	bash -f .ci/install_bats.sh
	bash -f integration/kubernetes/run_kubernetes_tests.sh

swarm:
	bash -f .ci/install_bats.sh
	cd integration/swarm && \
	bats swarm.bats

cri-containerd:
	bash -f .ci/install_cri_containerd.sh
	bash integration/containerd/cri/integration-tests.sh

log-parser:
	make -C cmd/log-parser

openshift:
	bash -f .ci/install_bats.sh
	bash -f integration/openshift/run_openshift_tests.sh

test: functional integration crio docker-compose openshift kubernetes swarm cri-containerd

check: checkcommits log-parser

.PHONY: \
	check \
	checkcommits \
	crio \
	docker-compose \
	functional \
	ginkgo \
	integration \
	kubernetes \
	log-parser \
	openshift \
	swarm \
	test
