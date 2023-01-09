# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04
ENV DEBIAN_FRONTEND=noninteractive
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
ARG RUST_TOOLCHAIN

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        clang \
        curl \
        gcc \
        git \
        llvm \
        nasm && \
    apt-get clean && rm -rf /var/lib/lists/ && \
    curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN} && \
    source "$HOME/.cargo/env" && \
    rustup component add rust-src && \
    cargo install cargo-xbuild
