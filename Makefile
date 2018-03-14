#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#


all: runtime

runtime:
	cd cli; \
	make
install :
	cd cli; \
	make install


help:
	@printf "To build a Kata Containers runtime:\n"
	@printf "\n"
	@printf "  \$$ make [install]\n"
	@printf "\n"
	@printf "Project home: https://github.com/kata-containers\n"
