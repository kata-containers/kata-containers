#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

default: checkcommits

checkcommits:
	make -C cmd/checkcommits
