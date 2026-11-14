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
# On s390x and ppc64le, LIBC=gnu is used instead of musl. Force openssl-sys to
# link OpenSSL statically (libssl.a/libcrypto.a) so the resulting binaries are
# fully static with no dynamic runtime dependencies.
ENV OPENSSL_STATIC=1

ENV PKG_CONFIG_PATH=${OPT_LIB}/pkgconfig:$PKG_CONFIG_PATH
# When Cargo compiles for explicit target triples (such as s390x-unknown-linux-gnu
# or powerpc64le-unknown-linux-gnu), pkg-config-rs treats the build as cross-compilation
# and refuses to invoke un-prefixed host pkg-config unless PKG_CONFIG_ALLOW_CROSS=1 is exported.
ENV PKG_CONFIG_ALLOW_CROSS=1

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME}

RUN apt-get update && \
	apt-get --no-install-recommends -y install \
		ca-certificates \
		clang \
		cmake \
		curl \
		g++ \
		gcc \
		git \
		gnupg \
		libssl-dev \
		make \
		musl-tools \
		openssl \
		perl \
		pkg-config \
		protobuf-compiler && \
	apt-get clean && rm -rf /var/lib/apt/lists/ && \
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN} && \
	chmod -R a+rwX /opt

# Install ORAS CLI for tarball caching
ARG ORAS_VERSION
RUN test -n "${ORAS_VERSION}" && ARCH=$(uname -m) && \
    case "${ARCH}" in \
        x86_64) ORAS_ARCH="amd64" ;; \
        aarch64) ORAS_ARCH="arm64" ;; \
        ppc64le) ORAS_ARCH="ppc64le" ;; \
        s390x) ORAS_ARCH="s390x" ;; \
        *) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
    esac && \
    curl -sSL "https://github.com/oras-project/oras/releases/download/v${ORAS_VERSION}/oras_${ORAS_VERSION}_linux_${ORAS_ARCH}.tar.gz" -o /tmp/oras.tar.gz && \
    tar -xzf /tmp/oras.tar.gz -C /usr/local/bin oras && \
    rm /tmp/oras.tar.gz && \
    oras version

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

RUN ARCH=$(uname -m); \
	goarch=""; \
	kernelname=$(uname -s | tr '[:upper:]' '[:lower:]'); \
	case "${ARCH}" in \
		"aarch64") goarch="arm64" ;; \
		"ppc64le") goarch="${ARCH}" ;; \
		"x86_64") goarch="amd64" ;; \
		"s390x") goarch="${ARCH}" ;; \
		*) echo "Unsupported architecture: ${ARCH}" && exit 1 ;; \
	esac; \
	curl -fsSOL "https://go.dev/dl/go${GO_TOOLCHAIN}.${kernelname}-${goarch}.tar.gz" && \
	tar -C "${GO_HOME}" -xzf "go${GO_TOOLCHAIN}.${kernelname}-${goarch}.tar.gz" && \
	rm "go${GO_TOOLCHAIN}.${kernelname}-${goarch}.tar.gz"
