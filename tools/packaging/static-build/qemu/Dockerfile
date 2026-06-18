# Copyright (c) 2019 Intel Corporation
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
FROM ubuntu:22.04

# CACHE_TIMEOUT: date to invalid cache, if the date changes the image will be rebuild
# This is required to keep build dependencies with security fixes.
ARG CACHE_TIMEOUT
ARG DEBIAN_FRONTEND=noninteractive
ARG DPKG_ARCH
ARG ARCH
ARG GCC_ARCH
ARG PREFIX

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN if [ "${ARCH}" != "$(uname -m)" ]; then sed -i 's/^deb/deb [arch=amd64]/g' /etc/apt/sources.list && \
    dpkg --add-architecture "${DPKG_ARCH#:}" && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ jammy main restricted" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ jammy-updates main restricted" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ jammy universe" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ jammy-updates universe" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ jammy multiverse" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ jammy-updates multiverse" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ jammy-backports main restricted universe multiverse" >> /etc/apt/sources.list; fi

RUN apt-get update && apt-get upgrade -y && \
    apt-get --no-install-recommends install -y \
	    apt-utils \
	    autoconf \
	    automake \
	    bc \
	    bison \
	    ca-certificates \
	    cpio \
	    dpkg-dev \
	    flex \
	    gawk \
	    libaio-dev${DPKG_ARCH} \
	    libaudit-dev${DPKG_ARCH} \
	    libblkid-dev${DPKG_ARCH} \
	    libcap-dev${DPKG_ARCH} \
	    libcap-ng-dev${DPKG_ARCH} \
	    libdw-dev${DPKG_ARCH} \
	    libelf-dev${DPKG_ARCH} \
	    libffi-dev${DPKG_ARCH} \
	    libglib2.0-0${DPKG_ARCH} \
	    libglib2.0-dev${DPKG_ARCH} \
	    libglib2.0-dev${DPKG_ARCH} git \
	    libltdl-dev${DPKG_ARCH} \
	    libmount-dev${DPKG_ARCH} \
	    libnuma-dev${DPKG_ARCH} \
	    libpixman-1-dev${DPKG_ARCH} \
	    libselinux1-dev${DPKG_ARCH} \
	    libtool${DPKG_ARCH} \
	    liburing-dev${DPKG_ARCH} \
	    make \
	    ninja-build \
	    pkg-config${DPKG_ARCH} \
	    libseccomp-dev${DPKG_ARCH} \
	    libseccomp2${DPKG_ARCH} \
	    patch \
	    python3 \
	    python3-dev \
	    python3-venv \
	    python3-tomli \
	    rsync \
	    zlib1g-dev${DPKG_ARCH} && \
    if [ "${ARCH}" != s390x ]; then apt-get install -y --no-install-recommends libpmem-dev${DPKG_ARCH}; fi && \
    GCC_ARCH="${ARCH}" && if [ "${ARCH}" = "ppc64le" ]; then GCC_ARCH="powerpc64le"; fi && \
    if [ "${ARCH}" != "$(uname -m)" ]; then apt-get install --no-install-recommends -y gcc-"${GCC_ARCH}"-linux-gnu; fi && \
    apt-get clean && rm -rf /var/lib/apt/lists/

# Detect architecture and set appropriate Ubuntu mirror
RUN ARCH=$(dpkg --print-architecture) && \
    if [ "${ARCH}" = "amd64" ]; then \
        UBUNTU_MIRROR="http://archive.ubuntu.com/ubuntu/"; \
    else \
        UBUNTU_MIRROR="http://ports.ubuntu.com/ubuntu-ports/"; \
    fi && \
    echo "UBUNTU_MIRROR=${UBUNTU_MIRROR}" > /etc/ubuntu_mirror.env

# Backup sources list and add Noble (24.04) repository
RUN . /etc/ubuntu_mirror.env && \
    cp /etc/apt/sources.list /etc/apt/sources.list.backup && \
    echo "deb ${UBUNTU_MIRROR} noble main universe" > /etc/apt/sources.list.d/noble.list

# Set up APT pinning to prefer Jammy packages by default, but allow liburing from Noble
RUN mkdir -p /etc/apt/preferences.d && \
    echo "Package: *\n\
Pin: release n=jammy\n\
Pin-Priority: 900\n\
\n\
Package: liburing*\n\
Pin: release n=noble\n\
Pin-Priority: 950" > /etc/apt/preferences.d/noble-pin

# Update package list and install liburing-dev from Noble
RUN apt-get update && \
    apt-get install -y liburing-dev && \
    rm -rf /var/lib/apt/lists/*

# Remove the Noble repository to prevent accidental upgrades
RUN rm /etc/apt/sources.list.d/noble.list && \
    apt-get update

# libigvm: QEMU's `--enable-igvm` (the igvm-cfg object the kata-qemu-snp-igvm
# runtime boots from) needs the IGVM C library exposed via pkg-config as `igvm`.
# It is not packaged, so build it from the upstream igvm crate's C API with
# cargo-c, pinned to the 0.4.x the runtime expects, and install libigvm.so +
# igvm.pc under /usr/local where QEMU's configure looks.
#
# TODO(confidential-dot-ai): confirm this matches how libigvm 0.4.0 was built
# for /opt/qemu-igvm (cargo-c vs the igvm_c Makefile, and the exact tag) — this
# is a reconstruction. amd64-only; the kata-qemu-snp-igvm class is x86_64-only.
ARG IGVM_VERSION=v0.4.0
RUN if [ "$(dpkg --print-architecture)" = "amd64" ]; then \
        apt-get update && apt-get install -y --no-install-recommends curl gcc libc6-dev && \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && \
        . "$HOME/.cargo/env" && \
        cargo install cargo-c --locked && \
        git clone --depth 1 --branch "${IGVM_VERSION}" https://github.com/microsoft/igvm /tmp/igvm && \
        cargo cinstall --release --manifest-path /tmp/igvm/igvm_c/Cargo.toml --prefix=/usr/local && \
        ldconfig && \
        pkg-config --exists igvm && echo "libigvm $(pkg-config --modversion igvm) installed" && \
        rm -rf /tmp/igvm "$HOME/.rustup" "$HOME/.cargo" && \
        apt-get clean && rm -rf /var/lib/apt/lists/* ; \
    fi
