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
