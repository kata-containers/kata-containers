#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# The time limit in seconds for each test
TIMEOUT := 60

# union for 'make test'
UNION := functional docker crio docker-compose network netmon docker-stability oci openshift kubernetes swarm vm-factory entropy ramdisk shimv2

# skipped test suites for docker integration tests
FILTER_FILE = .ci/hypervisors/$(KATA_HYPERVISOR)/filter_docker_$(KATA_HYPERVISOR).sh
ifneq ($(wildcard $(FILTER_FILE)),)
	SKIP := $(shell bash -f $(FILTER_FILE))
endif

# get arch
ARCH := $(shell bash -c '.ci/kata-arch.sh -d')

ARCH_DIR = arch
ARCH_FILE_SUFFIX = -options.mk
ARCH_FILE = $(ARCH_DIR)/$(ARCH)$(ARCH_FILE_SUFFIX)

# Number of processor units available
NPROCS := $(shell grep -c ^processor /proc/cpuinfo)
# Number of `go test` processes that ginkgo will spawn.
GINKGO_NODES := $(shell echo $(NPROCS)\-2 | bc)

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
endif

ifeq ($(KATA_HYPERVISOR),firecracker)
	./ginkgo -v -focus "${FOCUS}" -skip "${SKIP}" \
		./integration/docker/ -- -runtime=${RUNTIME} -timeout=${TIMEOUT}
else ifeq ($(ARCH),aarch64)
	./ginkgo -v -focus "${FOCUS}" -skip "${SKIP}" \
		./integration/docker/ -- -runtime=${RUNTIME} -timeout=${TIMEOUT}
else
	# Run tests in parallel, skip tests that need to be run serialized
	./ginkgo -nodes=${GINKGO_NODES} -v -focus "${FOCUS}" -skip "${SKIP}" -skip "\[Serial Test\]" \
		./integration/docker/ -- -runtime=${RUNTIME} -timeout=${TIMEOUT}
	# Now run serialized tests
	./ginkgo -v -focus "${FOCUS}" -focus "\[Serial Test\]" -skip "${SKIP}" \
		./integration/docker/ -- -runtime=${RUNTIME} -timeout=${TIMEOUT}
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
	export ITERATIONS=2 && export MAX_CONTAINERS=20 && ./soak_parallel_rm.sh
	cd integration/stability && ./bind_mount_linux.sh

kubernetes:
	bash -f .ci/install_bats.sh
	bash -f integration/kubernetes/run_kubernetes_tests.sh

swarm:
	systemctl is-active --quiet docker || sudo systemctl start docker
	bash -f .ci/install_bats.sh
	cd integration/swarm && \
	bats swarm.bats

shimv2:
	bash integration/containerd/shimv2/shimv2-tests.sh
	bash integration/containerd/shimv2/shimv2-factory-tests.sh

cri-containerd:
	bash integration/containerd/cri/integration-tests.sh

log-parser:
	make -C cmd/log-parser

oci:
	systemctl is-active --quiet docker || sudo systemctl start docker
	cd integration/oci_calls && \
	bash -f oci_call_test.sh

openshift:
	bash -f .ci/install_bats.sh
	bash -f integration/openshift/run_openshift_tests.sh

pentest:
	bash -f pentest/all.sh

vm-factory:
	bash -f integration/vm_factory/vm_templating_test.sh


network:
	systemctl is-active --quiet docker || sudo systemctl start docker
	bash -f .ci/install_bats.sh
	bats integration/network/macvlan/macvlan_driver.bats
	bats integration/network/ipvlan/ipvlan_driver.bats
	bats integration/network/disable_net/net_none.bats

ramdisk:
	bash -f integration/ramdisk/ramdisk.sh

entropy:
	bash -f .ci/install_bats.sh
	cd integration/entropy && \
	bats entropy_test.bats

netmon:
	systemctl is-active --quiet docker || sudo systemctl start docker
	bash -f .ci/install_bats.sh
	bats integration/netmon/netmon_test.bats

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
	entropy \
	functional \
	ginkgo \
	kubernetes \
	log-parser \
	oci \
	openshift \
	pentest \
	swarm \
	netmon \
	network \
	ramdisk \
	test \
	vm-factory
