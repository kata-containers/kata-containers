# Copyright (c) 2026 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive

ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:${PATH}"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ARG RUST_TOOLCHAIN

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME}

# td-shim's build needs NASM to assemble the ResetVector and libc6-dev for the
# C runtime startup objects (Scrt1.o, crti.o) gcc needs when linking the host
# tools; see the td-shim README "How to build" section. The vendored ring crate
# (secure boot) is not built, so clang/llvm-ar are not required.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        binutils \
        ca-certificates \
        curl \
        gcc \
        git \
        jq \
        libc6-dev \
        nasm && \
    apt-get clean && rm -rf /var/lib/apt/lists/* && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN} && \
    rustup target add x86_64-unknown-none && \
    chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME}
