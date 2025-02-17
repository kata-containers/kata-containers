# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive

ENV GO_HOME="/opt"
ENV GOCACHE="${GO_HOME}/.cache" 
ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:/opt/go/bin:${PATH}"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME} ${GOCACHE} && \
    chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME} ${GO_HOME}

ARG GO_VERSION
ARG RUST_VERSION

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        gcc \
        git \
        make \
        musl-tools \
        protobuf-compiler \
        sudo && \
        apt-get clean && rm -rf /var/lib/apt/lists/&& \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_VERSION}
    
RUN ARCH=$(uname -m); \
   	rust_arch=""; \
   	libc=""; \
   	case "${ARCH}" in \
        "aarch64") rust_arch="${ARCH}"; libc="musl"; ;; \
        "ppc64le") rust_arch="powerpc64le"; libc="gnu"; ;; \
        "x86_64") rust_arch="${ARCH}"; libc="musl"; ;; \
        "s390x") rust_arch="${ARCH}"; libc="gnu"; ;; \
        *) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
  	esac; \
   	rustup target add "${rust_arch}-unknown-linux-${libc}" 
        
RUN ARCH=$(uname -m); \
    goarch=""; \
    kernelname=$(uname -s | tr '[:upper:]' '[:lower:]'); \
   	case "${ARCH}" in \
        "aarch64") goarch="arm64" ;; \
        "ppc64le") goarch=${ARCH} ;; \
        "x86_64") goarch="amd64" ;; \
        "s390x") goarch=${ARCH} ;; \
        *) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
    esac; \
    curl -OL "https://storage.googleapis.com/golang/go${GO_VERSION}.${kernelname}-${goarch}.tar.gz" && \
    tar -C "${GO_HOME}" -xzf "go${GO_VERSION}.${kernelname}-${goarch}.tar.gz" && \
    rm "go${GO_VERSION}.${kernelname}-${goarch}.tar.gz"
