# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Set up an Ubuntu image with 'fio io tester' installed

FROM docker.io/library/ubuntu:22.04

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

# URL for the fio tester
ENV FIO_TOOLING_URL "https://github.com/axboe/fio"

RUN apt-get update --quiet && \
	apt-get install --quiet --no-install-recommends -y \
	bash \
	util-linux \
	procps \
	fio && \
	apt-get clean && \
	rm -rf /var/lib/apt/lists/

COPY workload/fio_bench.sh /
WORKDIR /
CMD ["/bin/bash"]
