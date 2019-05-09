#Copyright (c) 2019 Intel Corporation
#
#SPDX-License-Identifier: Apache-2.0
#

MK_DIR :=$(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
CONFIG_DIR := $(MK_DIR)/../../scripts/

build:
	"$(MK_DIR)/build-static-nemu.sh"

clean:
	rm -f kata-nemu-static.tar.gz
