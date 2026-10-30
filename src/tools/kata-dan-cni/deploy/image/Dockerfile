# syntax=docker/dockerfile:1
# Copyright (c) 2026 Kata Containers community
# SPDX-License-Identifier: Apache-2.0

FROM rust:1.97-slim-bullseye AS builder

# Build from the Kata repository root:
#   docker build -f src/tools/kata-dan-cni/deploy/image/Dockerfile .
# Mount the workspace read-only instead of copying unrelated source into a layer.
WORKDIR /src

RUN --mount=type=bind,source=.,target=/src \
    cargo build \
      --release \
      --locked \
      --target-dir /target \
      --package kata-dan-cni \
      --bins

FROM debian:bullseye-slim

COPY --from=builder \
  /target/release/kata-dan-cni \
  /kata-dan-cni/kata-dan-cni
COPY --from=builder \
  /target/release/render-cni-config \
  /kata-dan-cni/render-cni-config
COPY src/tools/kata-dan-cni/deploy/image/01-kata-dan.conflist.template \
  /kata-dan-cni/01-kata-dan.conflist.template
COPY --chmod=0755 \
  src/tools/kata-dan-cni/deploy/image/install-cni.sh \
  /kata-dan-cni/install-cni.sh
COPY --chmod=0755 \
  src/tools/kata-dan-cni/deploy/image/install-credentials.sh \
  /kata-dan-cni/install-credentials.sh
COPY --chmod=0755 \
  src/tools/kata-dan-cni/deploy/image/sync-credentials.sh \
  /kata-dan-cni/sync-credentials.sh
COPY --chmod=0755 \
  src/tools/kata-dan-cni/deploy/image/uninstall-cni.sh \
  /kata-dan-cni/uninstall-cni.sh

WORKDIR /kata-dan-cni
