# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
from ubuntu:18.04

ARG NEMU_REPO
ARG NEMU_VERSION
ARG NEMU_OVMF
ARG VIRTIOFSD_RELEASE
ARG VIRTIOFSD
ARG PREFIX

WORKDIR /root/nemu
RUN apt-get update
RUN apt-get install -y \
	    autoconf \
	    automake \
	    bc \
	    bison \
	    cpio \
	    flex \
	    gawk \
	    libaudit-dev \
	    libcap-dev \
	    libcap-ng-dev \
	    libdw-dev \
	    libelf-dev \
	    libglib2.0-0 \
	    libglib2.0-dev \
	    libglib2.0-dev git \
	    libltdl-dev \
	    libpixman-1-dev \
	    libtool \
	    pkg-config \
	    pkg-config \
	    python \
	    python-dev \
	    rsync \
	    wget \
	    zlib1g-dev

RUN cd  .. && git clone --depth=1 "${NEMU_REPO}" nemu
RUN git fetch origin --tags && git checkout "${NEMU_VERSION}"
RUN git clone https://github.com/qemu/capstone.git capstone
RUN git clone https://github.com/qemu/keycodemapdb.git ui/keycodemapdb

ADD configure-hypervisor.sh /root/configure-hypervisor.sh

RUN PREFIX="${PREFIX}" /root/configure-hypervisor.sh -s kata-nemu | xargs ./configure \
       --with-pkgversion=kata-static

RUN make -j$(nproc)
RUN make install DESTDIR=/tmp/nemu-static

RUN wget "${NEMU_OVMF}" && mv OVMF.fd /tmp/nemu-static/"${PREFIX}"/share/kata-nemu/
RUN mv /tmp/nemu-static/"${PREFIX}"/bin/qemu-system-x86_64 /tmp/nemu-static/"${PREFIX}"/bin/nemu-system-x86_64
RUN wget "${VIRTIOFSD_RELEASE}/${VIRTIOFSD}" && chmod +x ${VIRTIOFSD} && mv ${VIRTIOFSD} /tmp/nemu-static/"${PREFIX}"/bin/

RUN cd /tmp/nemu-static && tar -czvf kata-nemu-static.tar.gz *
