# Copyright (c) 2023 Intel
#
# SPDX-License-Identifier: Apache-2.0

FROM alpine:3.18
ARG GO_TOOLCHAIN
ARG RUST_TOOLCHAIN

ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:${PATH}"

ENV LIBC="musl"
ENV LIBSECCOMP_LINK_TYPE=static
ENV LIBSECCOMP_LIB_PATH=/usr/lib

SHELL ["/bin/ash", "-o", "pipefail", "-c"]

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME} && chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME}

RUN apk --no-cache add \
        bash \
        cmake \
        curl \
        gcc \
        git \
        libcap-ng-static \
        libseccomp-static \
        openssl-dev \
        openssl-libs-static \
        make \
        musl-dev \
        protoc && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}
