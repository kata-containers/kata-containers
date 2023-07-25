#
# Copyright (c) 2021-2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MKFILE_DIR := $(dir $(MKFILE_PATH))

build:
	GO111MODULE=on go build

run: build
	$(MKFILE_DIR)/fio-k8s --debug --fio.size 10M --output-dir test-results --test-name kata $(MKFILE_DIR)/../../configs/example-config/
	$(MKFILE_DIR)/fio-k8s --debug --fio.size 10M --output-dir test-results --test-name runc --container-runtime runc $(MKFILE_DIR)/../../configs/example-config/

gomod:
	 go mod edit -replace=github.com/kata-containers/tests/metrics/k8s=../../pkg/k8s
	 go mod edit -replace=github.com/kata-containers/tests/metrics/exec=../../pkg/exec
	 go mod edit -replace=github.com/kata-containers/tests/metrics/env=../../pkg/env
	 go mod tidy

runci: build
	$(MKFILE_DIR)/fio-k8s --debug --fio.size 10M --output-dir test-results --test-name kata $(MKFILE_DIR)/../../configs/example-config/
