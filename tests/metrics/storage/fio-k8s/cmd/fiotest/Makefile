#
# Copyright (c) 2021-2023 Intel Corporation
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
	 go mod edit -replace=github.com/kata-containers/kata-containers/tests/metrics/k8s=../../pkg/k8s
	 go mod edit -replace=github.com/kata-containers/kata-containers/tests/metrics/exec=../../pkg/exec
	 go mod edit -replace=github.com/kata-containers/kata-containers/tests/metrics/env=../../pkg/env
	 go mod tidy

run-qemu: build
	$(MKFILE_DIR)/fio-k8s --debug --fio.size 10M --output-dir test-results --test-name kata --container-runtime kata-qemu $(MKFILE_DIR)/../../configs/example-config/

run-clh: build
	$(MKFILE_DIR)/fio-k8s --debug --fio.size 10M --output-dir test-results --test-name kata --container-runtime kata-clh $(MKFILE_DIR)/../../configs/example-config/
