# Copyright (c) 2023 Intel
#
# SPDX-License-Identifier: Apache-2.0

FROM alpine:3.18
ARG RUST_TOOLCHAIN

SHELL ["/bin/ash", "-o", "pipefail", "-c"]
RUN apk --no-cache add \
        bash \
        curl \
        gcc \
        git \
        libcap-ng-static \
        libseccomp-static \
        make \
        musl-dev \
        openssl-dev \
        openssl-libs-static \
        protoc && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}