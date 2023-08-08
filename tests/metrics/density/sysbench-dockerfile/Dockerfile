# Copyright (c) 2022-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Usage: FROM [image name]
FROM ubuntu:20.04

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

RUN apt-get update && \
	apt-get install -y build-essential git curl sudo && \
	apt-get remove -y unattended-upgrades && \
	curl -OkL https://packagecloud.io/install/repositories/akopytov/sysbench/script.deb.sh && \
	apt-get install -y sysbench

CMD ["/bin/bash"]
