# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Set up an Ubuntu image with 'blogbench' installed

# Usage: FROM [image name]
FROM ubuntu

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

# URL for blogbench test and blogbench version
ENV BLOGBENCH_URL "https://download.pureftpd.org/pub/blogbench"
ENV BLOGBENCH_VERSION 1.1

RUN apt-get update && \
	apt-get install -y build-essential curl && \
	apt-get remove -y unattended-upgrades && \
	curl -OkL ${BLOGBENCH_URL}/blogbench-${BLOGBENCH_VERSION}.tar.gz && \
	tar xzf blogbench-${BLOGBENCH_VERSION}.tar.gz && \
	cd blogbench-${BLOGBENCH_VERSION} && \
	./configure && \
	make && \
	make install-strip

CMD ["/bin/bash"]
