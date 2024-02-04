# Copyright (c) 2023 Intel
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04
ARG RUST_TOOLCHAIN

COPY install_libseccomp.sh /usr/bin/install_libseccomp.sh

ENV DEBIAN_FRONTEND=noninteractive

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN apt-get update && \
	apt-get --no-install-recommends -y install \
		ca-certificates \
		curl \
		g++ \
		gcc \
		libssl-dev \
		make \
		musl-tools \
		openssl \
		perl \
		protobuf-compiler \
		clang && \
	apt-get clean && rm -rf /var/lib/apt/lists/ && \
    	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}

# aarch64 requires this name -- link for all
RUN ln -s /usr/bin/musl-gcc "/usr/bin/$(uname -m)-linux-musl-gcc"
