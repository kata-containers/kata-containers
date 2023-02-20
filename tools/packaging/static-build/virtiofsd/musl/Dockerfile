# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

FROM alpine:3.16.2
ARG RUST_TOOLCHAIN

SHELL ["/bin/ash", "-o", "pipefail", "-c"]
RUN apk --no-cache add \
        bash \
        curl \
        gcc \
        git \
        libcap-ng-static \
        libseccomp-static \
        musl-dev && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}
