# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        gcc \
        git \
        make \
        sudo && \
    apt-get clean && rm -rf /var/lib/apt/lists/

COPY install_go.sh /usr/bin/install_go.sh
ARG GO_VERSION
RUN install_go.sh "${GO_VERSION}"
ENV PATH=/usr/local/go/bin:${PATH}
