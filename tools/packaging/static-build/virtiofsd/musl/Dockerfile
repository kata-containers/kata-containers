# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

FROM alpine:3.16.2
ARG RUST_TOOLCHAIN

ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:${PATH}"

SHELL ["/bin/ash", "-o", "pipefail", "-c"]

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME} && chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME}

RUN apk --no-cache add \
        bash \
        curl \
        gcc \
        git \
        libcap-ng-static \
        libseccomp-static \
        musl-dev && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}

RUN ARCH=$(uname -m); \
    	rust_arch=""; \
    	libc=""; \
    	arch_libc=""; \
    	extra_rust_flags=" -C link-self-contained=yes"; \
        	case "${ARCH}" in \
    	        "aarch64") rust_arch="${ARCH}"; libc="musl"; arch_libc="" ;; \
    	        "ppc64le") rust_arch="powerpc64le"; libc="gnu"; arch_libc=${rust_arch}-linux-${libc}; extra_rust_flags="" ;; \
    	        "x86_64") rust_arch="${ARCH}"; libc="musl"; arch_libc="" ;; \
    	        "s390x") rust_arch="${ARCH}"; libc="gnu"; arch_libc=${rust_arch}-linux-${libc}; extra_rust_flags="" ;; \
    	        *) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
        	esac; \
        	echo "RUST_ARCH=${rust_arch}" > /etc/profile.d/rust.sh; \
    	echo "LIBC=${libc}" >> /etc/profile.d/rust.sh; \
    	echo "ARCH_LIBC=${arch_libc}" >> /etc/profile.d/rust.sh; \
    	echo "EXTRA_RUST_FLAGS=\"${extra_rust_flags}\"" >> /etc/profile.d/rust.sh;
