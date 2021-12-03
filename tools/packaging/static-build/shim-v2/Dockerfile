# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install -y \
        curl \
        gcc \
        git \
        make \
        sudo

ADD install_go.sh /usr/bin/install_go.sh
ARG GO_VERSION
RUN install_go.sh "${GO_VERSION}"
ENV PATH=/usr/local/go/bin:${PATH}
