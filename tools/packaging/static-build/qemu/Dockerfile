# Copyright (c) 2019 Intel Corporation
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
FROM ubuntu:22.04

# CACHE_TIMEOUT: date to invalid cache, if the date changes the image will be rebuild
# This is required to keep build dependencies with security fixes.
ARG CACHE_TIMEOUT
ARG DEBIAN_FRONTEND=noninteractive
ARG PREFIX

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

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
	    libaio-dev \
	    libaudit-dev \
	    libblkid-dev \
	    libcap-dev \
	    libcap-ng-dev \
	    libdw-dev \
	    libelf-dev \
	    libffi-dev \
	    libglib2.0-0 \
	    libglib2.0-dev \
	    libglib2.0-dev git \
	    libltdl-dev \
	    libmount-dev \
	    libnuma-dev \
	    libpixman-1-dev \
	    libselinux1-dev \
	    libtool \
	    liburing-dev \
	    make \
	    ninja-build \
	    pkg-config \
	    libseccomp-dev \
	    libseccomp2 \
	    patch \
	    python3 \
	    python3-dev \
	    python3-venv \
	    python3-tomli \
	    rsync \
	    zlib1g-dev && \
    if [ "$(uname -m)" != s390x ]; then apt-get install -y --no-install-recommends libpmem-dev; fi && \
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
