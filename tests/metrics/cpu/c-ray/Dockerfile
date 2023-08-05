# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Usage: FROM [image name]
FROM ubuntu:20.04

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

ENV DEBIAN_FRONTEND=noninteractive

# URL for c-ray benchmark
ENV CRAY_URL "http://www.phoronix-test-suite.com/benchmark-files/c-ray-1.1.tar.gz"

RUN apt-get update && \
	apt-get install -y --no-install-recommends build-essential gcc curl && \
	apt-get remove -y unattended-upgrades && \
	curl -OkL "${CRAY_URL}" && \
	tar -zxvf c-ray-1.1.tar.gz && \
	cd c-ray-1.1 && \
	cc -o c-ray-mt c-ray-mt.c -lm -lpthread && \
	make && \
	make install

CMD ["/bin/bash"]
