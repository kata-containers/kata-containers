# Copyright (c) 2019 Intel Corporation
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
from ubuntu:20.04

ARG QEMU_DESTDIR
ARG QEMU_REPO
# commit/tag/branch
ARG QEMU_VERSION
ARG QEMU_TARBALL
ARG PREFIX

WORKDIR /root/qemu

# CACHE_TIMEOUT: date to invalid cache, if the date changes the image will be rebuild
# This is required to keep build dependencies with security fixes.
ARG CACHE_TIMEOUT
RUN echo "$CACHE_TIMEOUT"

RUN apt-get update && apt-get upgrade -y
RUN apt-get --no-install-recommends install -y \
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
	    libpmem-dev \
	    libselinux1-dev \
	    libtool \
	    make \
	    ninja-build \
	    pkg-config \
	    libseccomp-dev \
	    libseccomp2 \
	    python \
	    python-dev \
	    rsync \
	    zlib1g-dev

RUN cd  .. && git clone "${QEMU_REPO}" qemu
RUN git checkout "${QEMU_VERSION}"
RUN git clone https://github.com/qemu/capstone.git capstone
RUN git clone https://github.com/qemu/keycodemapdb.git ui/keycodemapdb

ADD scripts/configure-hypervisor.sh /root/configure-hypervisor.sh
ADD qemu /root/kata_qemu
ADD scripts/apply_patches.sh /root/apply_patches.sh
ADD static-build /root/static-build

RUN echo "Apply patches for base version"
RUN stable_branch=$(cat VERSION | awk 'BEGIN{FS=OFS="."}{print $1 "." $2 ".x"}') && \
	/root/apply_patches.sh "/root/kata_qemu/patches/${stable_branch}"

RUN echo "Apply patches for specific qemu version"
RUN /root/apply_patches.sh "/root/kata_qemu/patches/tag_patches/${QEMU_VERSION}"
RUN PREFIX="${PREFIX}" /root/configure-hypervisor.sh -s kata-qemu | xargs ./configure \
	--with-pkgversion=kata-static

RUN make -j$(nproc)
RUN make install DESTDIR="${QEMU_DESTDIR}"
RUN /root/static-build/scripts/qemu-build-post.sh
