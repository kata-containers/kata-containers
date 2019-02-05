#
# Copyright (c) 2019 IBM
#
# SPDX-License-Identifier: Apache-2.0

# union for 'make test'
UNION := $(shell bash -f .ci/ppc64le/filter_test_ppc64le.sh)

# skiped test suites for docker integration tests
SKIP := $(shell bash -f .ci/ppc64le/filter_docker_ppc64le.sh)
