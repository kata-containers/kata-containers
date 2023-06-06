#
# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MKFILE_DIR := $(dir $(MKFILE_PATH))
run:
	$(MKFILE_DIR)/compare-virtiofsd-dax.sh
	"$(MKFILE_DIR)/report/gen-html-fio-report.sh" "./results"
