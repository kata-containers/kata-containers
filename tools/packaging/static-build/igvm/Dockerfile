# Copyright (c) 2026 Lunal / Confidential AI
#
# SPDX-License-Identifier: Apache-2.0
#
# Builder image for the measured IGVM packaging step. Provides steep's
# igvm-tools (the IGVM builder + measurement) plus ukify (used by igvm-tools
# `--cmdline` to wrap the guest kernel into a measured UKI).

FROM rust:1.90-bookworm

ARG STEEP_REPO
ARG STEEP_VERSION

# systemd-ukify + the systemd-boot EFI stub are needed by `igvm-tools build
# --cmdline` to assemble the UKI; binutils provides objcopy.
RUN apt-get update \
	&& apt-get install -y --no-install-recommends \
		git ca-certificates binutils systemd-ukify systemd-boot-efi \
	&& rm -rf /var/lib/apt/lists/*

# Build and install steep's igvm-tools CLI (build + measure subcommands).
RUN git clone "${STEEP_REPO}" /steep \
	&& cd /steep \
	&& git checkout "${STEEP_VERSION}" \
	&& cargo install --path crates/igvm-tools --root /usr/local

WORKDIR /workspace
