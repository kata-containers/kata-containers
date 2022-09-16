# Copyright (c) 2019 Intel Corporation
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
from ubuntu:20.04


WORKDIR /root/qemu

# CACHE_TIMEOUT: date to invalid cache, if the date changes the image will be rebuild
# This is required to keep build dependencies with security fixes.
ARG CACHE_TIMEOUT
RUN echo "$CACHE_TIMEOUT"
ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get upgrade -y && \
    apt-get --no-install-recommends install -y \
	    apt-utils \
	    autoconf \
	    automake \
	    bc \
	    bison \
	    ca-certificates \
	    cpio \
	    flex \
	    gawk \
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
	    libpixman-1-dev \
	    libselinux1-dev \
	    libtool \
	    make \
	    ninja-build \
	    pkg-config \
	    libseccomp-dev \
	    libseccomp2 \
	    patch \
	    python \
	    python-dev \
	    rsync \
	    zlib1g-dev && \
    if [ "$(uname -m)" != "s390x" ]; then apt-get install -y --no-install-recommends libpmem-dev; fi && \
    apt-get clean && rm -rf /var/lib/apt/lists/

ARG QEMU_REPO
# commit/tag/branch
ARG QEMU_VERSION
ARG PREFIX
# BUILD_SUFFIX is used by the qemu-build-post.sh script to
# properly rename non vanilla versions of the QEMU
ARG BUILD_SUFFIX
ARG HYPERVISOR_NAME
ARG PKGVERSION
ARG QEMU_DESTDIR
ARG QEMU_TARBALL

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN git clone  https://github.com/axboe/liburing/ ~/liburing && \
    cd ~/liburing && \
    git checkout tags/liburing-2.1 && \
    make && make install && ldconfig

COPY scripts/configure-hypervisor.sh /root/configure-hypervisor.sh
COPY qemu /root/kata_qemu
COPY scripts/apply_patches.sh /root/apply_patches.sh
COPY scripts/patch_qemu.sh /root/patch_qemu.sh
COPY static-build/scripts/qemu-build-post.sh /root/static-build/scripts/qemu-build-post.sh
COPY static-build/qemu.blacklist /root/static-build/qemu.blacklist

RUN git clone --depth=1 "${QEMU_REPO}" qemu && \
    cd qemu && \
    git fetch --depth=1 origin "${QEMU_VERSION}" && git checkout FETCH_HEAD && \
    scripts/git-submodule.sh update meson capstone && \
    /root/patch_qemu.sh "${QEMU_VERSION}" "/root/kata_qemu/patches" && \
    (PREFIX="${PREFIX}" /root/configure-hypervisor.sh -s "${HYPERVISOR_NAME}" | xargs ./configure \
	--with-pkgversion="${PKGVERSION}") && \
    make -j"$(nproc ${CI:+--ignore 1})" && \
    make install DESTDIR="${QEMU_DESTDIR}" && \
    /root/static-build/scripts/qemu-build-post.sh
