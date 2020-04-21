#
# Copyright (c) 2019 IBM
#
# SPDX-License-Identifier: Apache-2.0

# configuration file
CONFIG := .ci/ppc64le/configuration_ppc64le.yaml

# union for 'make test'
UNION := $(shell bash -c '.ci/filter/filter_test_union.sh $(CONFIG)')

# skiped test suites for docker integration tests
SKIP := $(shell bash -c '.ci/filter/filter_docker_test.sh $(CONFIG)')

