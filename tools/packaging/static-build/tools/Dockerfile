# Copyright (c) 2023 Intel
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ARG GO_TOOLCHAIN
ARG RUST_TOOLCHAIN

COPY install_libseccomp.sh /usr/bin/install_libseccomp.sh

ENV DEBIAN_FRONTEND=noninteractive

ENV GO_HOME="/opt"
ENV GOCACHE="${GO_HOME}/.cache"
ENV GOMODCACHE="${GO_HOME}/.modcache"
ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:/opt/go/bin:${PATH}"
ENV OPT_LIB="/opt/lib"

ENV LIBSECCOMP_LINK_TYPE=static
ENV LIBSECCOMP_LIB_PATH=${OPT_LIB}

ENV PKG_CONFIG_PATH=${OPT_LIB}/pkgconfig:$PKG_CONFIG_PATH

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME} && chmod -R a+rwX /opt

RUN apt-get update && \
	apt-get --no-install-recommends -y install \
		ca-certificates \
		clang \
		cmake \
		curl \
		g++ \
		gcc \
		git \
		libssl-dev \
		make \
		musl-tools \
		perl \
		protobuf-compiler && \
	apt-get clean && rm -rf /var/lib/apt/lists/ && \
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}

# Tools only build for x86_64
RUN rustup target add x86_64-unknown-linux-musl

RUN kernelname=$(uname -s | tr '[:upper:]' '[:lower:]'); \
    curl -OL "https://storage.googleapis.com/golang/go${GO_TOOLCHAIN}.${kernelname}-amd64.tar.gz" && \
    tar -C "${GO_HOME}" -xzf "go${GO_TOOLCHAIN}.${kernelname}-amd64.tar.gz" && \
    rm "go${GO_TOOLCHAIN}.${kernelname}-amd64.tar.gz"
