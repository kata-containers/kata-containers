# Copyright (c) 2019 Intel Corporation
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
from ubuntu:20.04

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
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ focal main restricted" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ focal-updates main restricted" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ focal universe" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ focal-updates universe" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ focal multiverse" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ focal-updates multiverse" >> /etc/apt/sources.list && \
    echo "deb [arch=${DPKG_ARCH#:}] http://ports.ubuntu.com/ focal-backports main restricted universe multiverse" >> /etc/apt/sources.list; fi

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
	    make \
	    ninja-build \
	    pkg-config${DPKG_ARCH} \
	    libseccomp-dev${DPKG_ARCH} \
	    libseccomp2${DPKG_ARCH} \
	    patch \
	    python \
	    python-dev \
	    rsync \
	    zlib1g-dev${DPKG_ARCH} && \
    if [ "${ARCH}" != s390x ]; then apt-get install -y --no-install-recommends libpmem-dev${DPKG_ARCH}; fi && \
	if [ "${ARCH}" == ppc64le ]; then apt-get install -y --no-install-recommends librados-dev librbd-dev; fi && \
    GCC_ARCH="${ARCH}" && if [ "${ARCH}" = "ppc64le" ]; then GCC_ARCH="powerpc64le"; fi && \
    if [ "${ARCH}" != "$(uname -m)" ]; then apt-get install --no-install-recommends -y gcc-"${GCC_ARCH}"-linux-gnu; fi && \
    apt-get clean && rm -rf /var/lib/apt/lists/

RUN git clone  https://github.com/axboe/liburing/ ~/liburing && \
    cd ~/liburing && \
    git checkout tags/liburing-2.1 && \
    GCC_ARCH="${ARCH}" && if [ "${ARCH}" = "ppc64le" ]; then GCC_ARCH="powerpc64le"; fi && \
    if [ "${ARCH}" != "$(uname -m)" ]; then PREFIX="${GCC_ARCH}-linux-gnu"; fi && \
    ./configure --cc=${GCC_ARCH}-linux-gnu-gcc --cxx=${GCC_ARCH}-linux-gnu-cpp --prefix=/usr/${PREFIX}/ && \
    make && make install && ldconfig
