# Copyright (c) 2018-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Set up an Ubuntu image with 'blogbench' installed

# Usage: FROM [image name]
# hadolint ignore=DL3007
FROM docker.io/library/ubuntu:latest

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

ENV DEBIAN_FRONTEND=noninteractive

# URL for blogbench test and blogbench version
ENV BLOGBENCH_URL "https://download.pureftpd.org/pub/blogbench"
ENV BLOGBENCH_VERSION 1.2

RUN apt-get update && \
	apt-get install -y --no-install-recommends build-essential curl && \
	apt-get remove -y unattended-upgrades && \
	apt-get clean && \
	rm -rf /var/lib/apt/lists/ && \
	curl -OkL "${BLOGBENCH_URL}/blogbench-${BLOGBENCH_VERSION}.tar.gz" && \
	tar xzf "blogbench-${BLOGBENCH_VERSION}.tar.gz" -C /
WORKDIR "/blogbench-${BLOGBENCH_VERSION}"
RUN arch="$(uname -m)" && \
	export arch && \
	./configure --build="${arch}" && \
	make && \
	make install-strip

CMD ["/bin/bash"]
