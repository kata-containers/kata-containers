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
        musl-tools \
        protobuf-compiler \
        sudo && \
    apt-get clean && rm -rf /var/lib/apt/lists/

COPY install_go_rust.sh /usr/bin/install_go_rust.sh
ARG GO_VERSION
ARG RUST_VERSION
RUN install_go_rust.sh "${GO_VERSION}" "${RUST_VERSION}"
ENV PATH=/usr/local/go/bin:${PATH}
ENV PATH=/root/.cargo/bin/:${PATH}
