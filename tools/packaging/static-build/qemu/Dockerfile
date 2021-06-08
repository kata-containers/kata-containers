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

ARG QEMU_REPO

RUN cd  .. && git clone "${QEMU_REPO}" qemu

# commit/tag/branch
ARG QEMU_VERSION

RUN git checkout "${QEMU_VERSION}"
RUN git clone https://github.com/qemu/capstone.git capstone
RUN git clone https://github.com/qemu/keycodemapdb.git ui/keycodemapdb
RUN git clone https://github.com/qemu/meson.git meson
RUN git clone https://github.com/qemu/berkeley-softfloat-3.git tests/fp/berkeley-softfloat-3
RUN git clone https://github.com/qemu/berkeley-testfloat-3.git tests/fp/berkeley-testfloat-3

ADD scripts/configure-hypervisor.sh /root/configure-hypervisor.sh
ADD qemu /root/kata_qemu
ADD scripts/apply_patches.sh /root/apply_patches.sh
ADD scripts/patch_qemu.sh /root/patch_qemu.sh

RUN /root/patch_qemu.sh "${QEMU_VERSION}" "/root/kata_qemu/patches"

ARG PREFIX
ARG BUILD_SUFFIX
RUN PREFIX="${PREFIX}" /root/configure-hypervisor.sh -s "kata-qemu${BUILD_SUFFIX}" | xargs ./configure \
	--with-pkgversion="kata-static${BUILD_SUFFIX}"

RUN make -j$(nproc)
ARG QEMU_DESTDIR
RUN make install DESTDIR="${QEMU_DESTDIR}"
ARG QEMU_TARBALL
ADD static-build/scripts/qemu-build-post.sh /root/static-build/scripts/qemu-build-post.sh
ADD static-build/qemu.blacklist /root/static-build/qemu.blacklist
RUN /root/static-build/scripts/qemu-build-post.sh
