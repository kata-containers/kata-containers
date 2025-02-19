# Copyright (c) 2024 Intel
# Copyright (c) 2024 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ARG RUST_TOOLCHAIN

ENV DEBIAN_FRONTEND=noninteractive

ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:${PATH}"

# Note - the TDX lib is only available on x86, so there is an arch check in the package install
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME} && chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME}

RUN apt-get update && \
	apt-get --no-install-recommends install -y \
	ca-certificates \
	curl \
	gnupg && \
	if [ "$(uname -m)" == "x86_64" ]; then curl -sL https://download.01.org/intel-sgx/sgx_repo/ubuntu/intel-sgx-deb.key | gpg --dearmor --output /usr/share/keyrings/intel-sgx.gpg && \
	echo 'deb [arch=amd64 signed-by=/usr/share/keyrings/intel-sgx.gpg] https://download.01.org/intel-sgx/sgx_repo/ubuntu jammy main' | tee /etc/apt/sources.list.d/intel-sgx.list && \
	apt-get update && \
	apt-get --no-install-recommends -y install libtdx-attest-dev; fi && \
	apt-get --no-install-recommends -y install \
	binutils \
	clang \
	g++ \
	gcc \
	git \
	libssl-dev \
	libtss2-dev \
	make \
	musl-tools \
	openssl \
	perl \
	pkg-config \
	protobuf-compiler && \
	apt-get clean && rm -rf /var/lib/apt/lists/ && \
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}

ENV LIBC="gnu"
RUN ARCH=$(uname -m); \
	rust_arch=""; \
    	case "${ARCH}" in \
	        "aarch64") rust_arch="${ARCH}" ;; \
	        "ppc64le") rust_arch="powerpc64le" ;; \
	        "x86_64") rust_arch="${ARCH}" ;; \
	        "s390x") rust_arch="${ARCH}" ;; \
	        *) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
    	esac; \
    	echo "RUST_ARCH=${rust_arch}" > /etc/profile.d/rust.sh; \
	rustup target add "${rust_arch}-unknown-linux-${LIBC}"
