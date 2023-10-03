# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
	apt-get install -y --no-install-recommends sudo build-essential curl && \
	apt-get remove -y unattended-upgrades && \
	apt-get install -y stress stress-ng

CMD ["/bin/bash"]
