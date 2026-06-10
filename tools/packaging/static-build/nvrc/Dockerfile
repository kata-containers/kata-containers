# Copyright (c) 2024 NVIDIA Corporation
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
	gcc \
	libc6-dev \
	git \
	make \
	musl-tools && \
	apt-get clean && rm -rf /var/lib/apt/lists/ && \
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}

# NVRC is shipped as a fully static musl binary so it can serve as init in the
# minimal GPU guest regardless of the rootfs libc.
RUN ARCH=$(uname -m); \
	rust_arch=""; \
	case "${ARCH}" in \
		"aarch64") rust_arch="${ARCH}" ;; \
		"x86_64") rust_arch="${ARCH}" ;; \
		*) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
	esac; \
	echo "RUST_ARCH=${rust_arch}" > /etc/profile.d/rust.sh; \
	rustup target add "${rust_arch}-unknown-linux-musl"

RUN chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME}
