#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# The time limit in seconds for each test
TIMEOUT := 60

# union for 'make test'
UNION := functional docker crio docker-compose openshift kubernetes swarm cri-containerd

# skipped test suites for docker integration tests
SKIP :=

# get arch
ARCH := $(shell bash -c '.ci/kata-arch.sh -d')

ARCH_DIR = arch
ARCH_FILE_SUFFIX = -options.mk
ARCH_FILE = $(ARCH_DIR)/$(ARCH)$(ARCH_FILE_SUFFIX)

# Load architecture-dependent settings
ifneq ($(wildcard $(ARCH_FILE)),)
include $(ARCH_FILE)
endif

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

docker: ginkgo
ifeq ($(RUNTIME),)
	$(error RUNTIME is not set)
else
	./ginkgo -v -focus "${FOCUS}" -skip "${SKIP}" ./integration/docker/ -- -runtime=${RUNTIME} -timeout=${TIMEOUT}
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
	systemctl is-active --quiet docker || sudo systemctl start docker
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

test: ${UNION}

check: checkcommits log-parser

.PHONY: \
	check \
	checkcommits \
	crio \
	docker-compose \
	functional \
	ginkgo \
	docker \
	kubernetes \
	log-parser \
	openshift \
	swarm \
	test
