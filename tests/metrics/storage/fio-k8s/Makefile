#
# Copyright (c) 2021-2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MKFILE_DIR := $(dir $(MKFILE_PATH))

build:
	make -C $(MKFILE_DIR)/cmd/fiotest/ gomod
	make -C $(MKFILE_DIR)/cmd/fiotest/ build

test-report:
	$(MKFILE_DIR)/scripts/dax-compare-test/report/gen-html-fio-report.sh $(MKFILE_DIR)/cmd/fiotest/test-results/

test-report-interactive:
	$(MKFILE_DIR)/scripts/dax-compare-test/report/run-docker-jupyter-server.sh $(MKFILE_DIR)/cmd/fiotest/test-results/

test: build
	make -C $(MKFILE_DIR)/cmd/fiotest/ run
	make test-report

run: build
	make -C $(MKFILE_DIR)/scripts/dax-compare-test/ run

test-ci: build
	make -C $(MKFILE_DIR)/cmd/fiotest/ runci
