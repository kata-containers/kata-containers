# Copyright (c) 2024 Intel
# Copyright (c) 2024 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:24.04
ARG RUST_TOOLCHAIN

ENV DEBIAN_FRONTEND=noninteractive

ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:${PATH}"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME}

RUN apt-get update && \
	apt-get --no-install-recommends install -y \
	ca-certificates \
	curl \
	binutils \
	clang \
	g++ \
	gcc \
	git \
	libcryptsetup-dev \
	libssl-dev \
	libtss2-dev \
	make \
	cmake \
	musl-tools \
	openssl \
	perl \
	pkg-config \
	protobuf-compiler && \
	apt-get clean && rm -rf /var/lib/apt/lists/ && \
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}

ARG NVAT_VERSION
RUN if [ "$(uname -m)" = "x86_64" ] && [ -n "${NVAT_VERSION}" ]; then \
	apt-get update && apt-get --no-install-recommends install -y \
	build-essential libxml2-dev zlib1g-dev && \
	tmpdir=$(mktemp -d) && pushd "$tmpdir" && \
	git clone https://github.com/NVIDIA/attestation-sdk && \
	pushd attestation-sdk && git checkout "${NVAT_VERSION}" && \
	pushd nv-attestation-sdk-cpp && cmake . && make install && \
	mkdir -p /usr/include && ln -sf /usr/local/include/nvat.h /usr/include/nvat.h && ldconfig && \
	popd && popd && popd && rm -rf "$tmpdir" && \
	apt-get clean && rm -rf /var/lib/apt/lists/; fi

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

RUN chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME}
