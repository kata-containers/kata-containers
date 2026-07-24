# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive

ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="/opt/cargo/bin/:${PATH}"

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN mkdir ${RUSTUP_HOME} ${CARGO_HOME}

# OpenVMM build dependencies (see OpenVMM Guide: getting started on Linux) plus
# the tooling needed to clone the repo and bootstrap rustup. OpenVMM has no
# rust-toolchain.toml, so build-static-openvmm.sh installs and pins OpenVMM's
# declared MSRV (Cargo.toml rust-version) at build time; only a bootstrap
# rustup toolchain is installed here.
#
# `sudo` is needed because OpenVMM's `cargo xflowey restore-packages` installs
# additional build inputs at run time via `sudo apt-get`
# (flowey_lib_common::install_dist_pkg). Other Kata builders run as the invoking
# user and install every dependency here at image-build time; this builder
# instead runs as root (build.sh omits --user) so that the run-time
# `sudo apt-get` works, and build.sh chowns the outputs back to the caller.
#
# restore-packages drives apt with interactive=true (no `-y`), so make apt
# assume "yes" image-wide; otherwise those run-time installs hang on the
# "Do you want to continue? [Y/n]" prompt in CI (no TTY) and abort.
RUN echo 'APT::Get::Assume-Yes "true";' > /etc/apt/apt.conf.d/90kata-assume-yes && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
        binutils \
        build-essential \
        ca-certificates \
        curl \
        gcc-aarch64-linux-gnu \
        git \
        libssl-dev \
        pkg-config \
        sudo \
        unzip && \
    apt-get clean && rm -rf /var/lib/apt/lists/* && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable && \
    chmod -R a+rwX ${RUSTUP_HOME} ${CARGO_HOME}
