# Copyright Intel Corporation, 2022 IBM Corp.
# Copyright (c) 2025 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

#### Nydus snapshotter & nydus image

FROM golang:1.24-alpine AS nydus-binary-downloader

COPY versions.yaml /tmp/versions.yaml

RUN \
	set -e && \
	apk add --no-cache curl yq-go && \
	NYDUS_SNAPSHOTTER_VERSION="$(yq eval -e '.externals.nydus-snapshotter.version | explode(.)' /tmp/versions.yaml)" && \
	NYDUS_SNAPSHOTTER_REPO="$(yq eval -e '.externals.nydus-snapshotter.url | explode(.)' /tmp/versions.yaml)" && \
	mkdir -p /opt/nydus-snapshotter && \
	ARCH="$(uname -m)" && \
	if [ "${ARCH}" = "x86_64" ]; then ARCH=amd64 ; fi && \
	if [ "${ARCH}" = "aarch64" ]; then ARCH=arm64; fi && \
	curl -fOL --progress-bar "${NYDUS_SNAPSHOTTER_REPO}/releases/download/${NYDUS_SNAPSHOTTER_VERSION}/nydus-snapshotter-${NYDUS_SNAPSHOTTER_VERSION}-linux-${ARCH}.tar.gz" && \
	tar xvzpf "nydus-snapshotter-${NYDUS_SNAPSHOTTER_VERSION}-linux-${ARCH}.tar.gz" -C /opt/nydus-snapshotter && \
	rm "nydus-snapshotter-${NYDUS_SNAPSHOTTER_VERSION}-linux-${ARCH}.tar.gz"

#### Build binary package
FROM ubuntu:22.04 AS rust-builder

# Default to Rust 1.90.0
ARG RUST_TOOLCHAIN=1.90.0
ARG GITHUB_ACTIONS=false
ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:${PATH}"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY tools/packaging/docker/apt-ci-tune.sh /tmp/apt-ci-tune.sh
RUN GITHUB_ACTIONS="${GITHUB_ACTIONS}" bash /tmp/apt-ci-tune.sh && rm -f /tmp/apt-ci-tune.sh

RUN \
	mkdir ${RUSTUP_HOME} ${CARGO_HOME} && \
	chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME}

RUN \
	apt-get update && \
	apt-get --no-install-recommends -y install \
		ca-certificates \
		curl \
		gcc \
		libc6-dev \
		musl-tools && \
	apt-get clean && rm -rf /var/lib/apt/lists/* && \
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN} && \
	rustup component add rustfmt clippy

# Build from the repository root so kata-deploy uses the root Cargo workspace:
#   docker build -f tools/packaging/kata-deploy/Dockerfile .
WORKDIR /kata

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tools/packaging/kata-deploy/binary ./tools/packaging/kata-deploy/binary

# Install target and run tests based on architecture
# - AMD64/arm64: use musl for fully static binaries
# - PPC64le/s390x: use glibc (musl has issues on these platforms)
RUN \
	HOST_ARCH="$(uname -m)"; \
	rust_arch=""; \
	rust_target=""; \
	case "${HOST_ARCH}" in \
		"x86_64") \
			rust_arch="x86_64"; \
			rust_target="${rust_arch}-unknown-linux-musl"; \
			echo "Installing musl target for ${rust_target}"; \
			rustup target add "${rust_target}"; \
			;; \
		"aarch64") \
			rust_arch="aarch64"; \
			rust_target="${rust_arch}-unknown-linux-musl"; \
			echo "Installing musl target for ${rust_target}"; \
			rustup target add "${rust_target}"; \
			;; \
		"ppc64le") \
			rust_arch="powerpc64le"; \
			rust_target="${rust_arch}-unknown-linux-gnu"; \
			echo "Using glibc target for ${rust_target} (musl is not well supported on ppc64le)"; \
			;; \
		"s390x") \
			rust_arch="s390x"; \
			rust_target="${rust_arch}-unknown-linux-gnu"; \
			echo "Using glibc target for ${rust_target} (musl is not well supported on s390x)"; \
			;; \
		*) echo "Unsupported architecture: ${HOST_ARCH}" && exit 1 ;; \
	esac; \
	echo "${rust_target}" > /tmp/rust_target

# Verify code formatting and run cargo check before tests and build
RUN \
	set -e && \
	rust_target="$(cat /tmp/rust_target)" && \
	echo "Checking code formatting..." && \
	cargo fmt -p kata-deploy --check && \
	echo "Code formatting check passed!" && \
	echo "Running cargo clippy with target ${rust_target}..." && \
	cargo clippy -p kata-deploy --all-targets --all-features --release --locked --target "${rust_target}" -- -D warnings && \
	echo "Cargo clippy passed!"

# Run tests using --test-threads=1 to prevent environment variable pollution between tests,
# and this is fine as we'll never ever have multiple binaries running at the same time.
RUN \
	rust_target="$(cat /tmp/rust_target)"; \
	echo "Running binary tests with target ${rust_target}..." && \
	RUSTFLAGS="-D warnings" cargo test -p kata-deploy --target "${rust_target}" -- --test-threads=1 && \
	echo "All tests passed!"

RUN \
	rust_target="$(cat /tmp/rust_target)"; \
	echo "Building kata-deploy binary for ${rust_target}..." && \
	RUSTFLAGS="-D warnings" cargo build --release -p kata-deploy --target "${rust_target}" && \
	mkdir -p /kata-deploy/bin && \
	cp "/kata/target/${rust_target}/release/kata-deploy" /kata-deploy/bin/kata-deploy && \
	echo "Cleaning up build artifacts to save disk space..." && \
	rm -rf /kata/target && \
	cargo clean

#### Extract kata artifacts
FROM alpine:3.22 AS artifact-extractor

ARG KATA_ARTIFACTS=tools/packaging/kata-deploy/kata-static.tar.zst
ARG DESTINATION=/opt/kata-artifacts

COPY ${KATA_ARTIFACTS} /tmp/
RUN \
	apk add --no-cache tar zstd util-linux-misc && \
	mkdir -p "${DESTINATION}" && \
	tar --zstd -xf "/tmp/$(basename "${KATA_ARTIFACTS}")" -C "${DESTINATION}" && \
	rm -f "/tmp/$(basename "${KATA_ARTIFACTS}")"

#### Prepare runtime dependencies (nsenter and required libraries)
# This stage assembles all runtime dependencies based on architecture
# using ldd to find exact library dependencies
FROM debian:trixie-slim AS runtime-assembler

ARG DESTINATION=/opt/kata-artifacts
ARG GITHUB_ACTIONS=false

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

COPY tools/packaging/docker/apt-ci-tune.sh /tmp/apt-ci-tune.sh
RUN GITHUB_ACTIONS="${GITHUB_ACTIONS}" bash /tmp/apt-ci-tune.sh && rm -f /tmp/apt-ci-tune.sh

RUN \
	apt-get update && \
	apt-get --no-install-recommends -y install \
		util-linux && \
	apt-get clean && rm -rf /var/lib/apt/lists/*

# Copy the built binary to analyze its dependencies
COPY --from=rust-builder /kata-deploy/bin/kata-deploy /tmp/kata-deploy

# Create output directories
RUN mkdir -p /output/lib /output/lib64 /output/usr/bin

# Use ldd to find and copy all required libraries for the kata-deploy binary and nsenter
RUN \
	HOST_ARCH="$(uname -m)"; \
	echo "Preparing runtime dependencies for ${HOST_ARCH}"; \
	case "${HOST_ARCH}" in \
		"ppc64le"|"s390x") \
			echo "Using glibc - copying libraries based on ldd output"; \
			\
			# Copy nsenter \
			cp /usr/bin/nsenter /output/usr/bin/nsenter; \
			\
			# Show what the binaries need \
			echo "Libraries needed by kata-deploy:"; \
			ldd /tmp/kata-deploy || echo "ldd failed"; \
			echo "Libraries needed by nsenter:"; \
			ldd /usr/bin/nsenter || echo "ldd failed"; \
			\
			# Extract and copy all library paths from both binaries \
			for binary in /tmp/kata-deploy /usr/bin/nsenter; do \
				echo "Processing ${binary}..."; \
				# Get libraries with "=>" (shared libs) \
				ldd "${binary}" 2>/dev/null | grep "=>" | awk '{print $3}' | sort -u | while read -r lib; do \
					if [ -n "${lib}" ] && [ -f "${lib}" ]; then \
						dest_dir="/output$(dirname "${lib}")"; \
						mkdir -p "${dest_dir}"; \
						cp -Ln "${lib}" "${dest_dir}/" 2>/dev/null || true; \
						echo "  Copied lib: ${lib}"; \
					fi; \
				done; \
			done; \
			\
			# Copy the dynamic linker - it's at /lib/ld64.so.1 (not /lib64/) \
			echo "Copying dynamic linker:"; \
			mkdir -p /output/lib; \
			cp -Ln /lib/ld64.so* /output/lib/ 2>/dev/null || true; \
			cp -Ln /lib64/ld64.so* /output/lib64/ 2>/dev/null || true; \
			\
			echo "glibc" > /output/.libc-type; \
			;; \
		*) \
			echo "amd64/arm64: will use musl-based static binaries"; \
			echo "musl" > /output/.libc-type; \
			# Create placeholder so COPY doesn't fail \
			touch /output/lib/.placeholder; \
			touch /output/lib64/.placeholder; \
			touch /output/usr/bin/.placeholder; \
			;; \
	esac

# Copy musl nsenter from alpine for amd64/arm64
COPY --from=artifact-extractor /usr/bin/nsenter /output/usr/bin/nsenter-musl
COPY --from=artifact-extractor /lib/ld-musl-*.so.1 /output/lib/

# For amd64/arm64, use the musl nsenter; for ppc64le/s390x, keep the glibc one
RUN \
	HOST_ARCH="$(uname -m)"; \
	case "${HOST_ARCH}" in \
		"x86_64"|"aarch64") \
			mv /output/usr/bin/nsenter-musl /output/usr/bin/nsenter; \
			;; \
		*) \
			rm -f /output/usr/bin/nsenter-musl; \
			;; \
	esac

#### kata-deploy main image
FROM gcr.io/distroless/static-debian13@sha256:972618ca78034aaddc55864342014a96b85108c607372f7cbd0dbd1361f1d841

ARG DESTINATION=/opt/kata-artifacts

# Copy extracted kata artifacts
COPY --from=artifact-extractor ${DESTINATION} ${DESTINATION}

# Copy Rust binary
COPY --from=rust-builder /kata-deploy/bin/kata-deploy /usr/bin/kata-deploy

# Copy nsenter and required libraries (assembled based on architecture)
COPY --from=runtime-assembler /output/usr/bin/nsenter /usr/bin/nsenter
COPY --from=runtime-assembler /output/lib/ /lib/
COPY --from=runtime-assembler /output/lib64/ /lib64/

# Copy nydus snapshotter
COPY tools/packaging/kata-deploy/nydus-snapshotter ${DESTINATION}/nydus-snapshotter
COPY --from=nydus-binary-downloader /opt/nydus-snapshotter/bin/containerd-nydus-grpc ${DESTINATION}/nydus-snapshotter/
COPY --from=nydus-binary-downloader /opt/nydus-snapshotter/bin/nydus-overlayfs ${DESTINATION}/nydus-snapshotter/

# Copy runtimeclasses and node-feature-rules
COPY tools/packaging/kata-deploy/node-feature-rules ${DESTINATION}/node-feature-rules

ENTRYPOINT ["/usr/bin/kata-deploy"]
