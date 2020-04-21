#
# Copyright (c) 2019 IBM
#
# SPDX-License-Identifier: Apache-2.0

# configuration file
CONFIG := .ci/s390x/configuration_s390x.yaml

# union for 'make test'
UNION := $(shell bash -c '.ci/filter/filter_test_union.sh $(CONFIG)')

# skiped test suites for docker integration tests
SKIP := $(shell bash -c '.ci/filter/filter_docker_test.sh $(CONFIG)')

