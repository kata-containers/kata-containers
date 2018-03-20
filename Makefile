#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

all: runtime

runtime:
	make -C cli

install:
	make -C cli install

help:
	@printf "To build a Kata Containers runtime:\n"
	@printf "\n"
	@printf "  \$$ make [install]\n"
	@printf "\n"
	@printf "Project home: https://github.com/kata-containers\n"
