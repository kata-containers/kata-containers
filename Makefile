#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# The time limit in seconds for each test
TIMEOUT := 60

# union for 'make test'
UNION := functional docker crio docker-compose docker-stability openshift kubernetes swarm vm-factory ramdisk

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
	bash sanity/check_sanity.sh
endif

docker: ginkgo
ifeq ($(RUNTIME),)
	$(error RUNTIME is not set)
else
	./ginkgo -v -focus "${FOCUS}" -skip "${SKIP}" ./integration/docker/ -- -runtime=${RUNTIME} -timeout=${TIMEOUT}
	bash sanity/check_sanity.sh
endif

crio:
	bash .ci/install_bats.sh
	RUNTIME=${RUNTIME} ./integration/cri-o/cri-o.sh

docker-compose:
	bash .ci/install_bats.sh
	cd integration/docker-compose && \
	bats docker-compose.bats

docker-stability:
	systemctl is-active --quiet docker || sudo systemctl start docker
	cd integration/stability && \
	export ITERATIONS=2 && export MAX_CONTAINERS=20 && chronic ./soak_parallel_rm.sh

kubernetes:
	bash -f .ci/install_bats.sh
	bash -f integration/kubernetes/run_kubernetes_tests.sh

swarm:
	systemctl is-active --quiet docker || sudo systemctl start docker
	bash -f .ci/install_bats.sh
	cd integration/swarm && \
	bats swarm.bats

cri-containerd:
	bash integration/containerd/cri/integration-tests.sh

log-parser:
	make -C cmd/log-parser

openshift:
	bash -f .ci/install_bats.sh
	bash -f integration/openshift/run_openshift_tests.sh

pentest:
	bash -f pentest/all.sh

vm-factory:
	bash -f integration/vm_factory/vm_templating_test.sh


network:
	bash -f .ci/install_bats.sh
	bats integration/network/macvlan/macvlan_driver.bats
	bats integration/network/ipvlan/ipvlan_driver.bats

ramdisk:
	bash -f integration/ramdisk/ramdisk.sh

test: ${UNION}

check: checkcommits log-parser

# PHONY in alphabetical order
.PHONY: \
	check \
	checkcommits \
	crio \
	docker \
	docker-compose \
	docker-stability \
	functional \
	ginkgo \
	kubernetes \
	log-parser \
	openshift \
	pentest \
	swarm \
	network \
	ramdisk \
	test \
	vm-factory
