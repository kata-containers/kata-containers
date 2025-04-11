# Copyright (c) 2023 Intel
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ARG RUST_TOOLCHAIN

COPY install_libseccomp.sh /usr/bin/install_libseccomp.sh

ENV DEBIAN_FRONTEND=noninteractive

ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:${PATH}"
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
		curl \
		g++ \
		gcc \
		libprotobuf-dev \
		libssl-dev \
		make \
		musl-tools \
		openssl \
		perl \
		protobuf-compiler && \
	apt-get clean && rm -rf /var/lib/apt/lists/ && \
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}

RUN ARCH=$(uname -m); \
	rust_arch=""; \
	libc=""; \
    	case "${ARCH}" in \
	        "aarch64") rust_arch="${ARCH}"; libc="musl" ;; \
	        "ppc64le") rust_arch="powerpc64le"; libc="gnu" ;; \
	        "x86_64") rust_arch="${ARCH}"; libc="musl" ;; \
	        "s390x") rust_arch="${ARCH}"; libc="gnu" ;; \
	        *) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
    	esac; \
	rustup target add "${rust_arch}-unknown-linux-${libc}"
