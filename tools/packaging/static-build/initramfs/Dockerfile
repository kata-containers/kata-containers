# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
FROM ubuntu:22.04

ARG DEBIAN_FRONTEND=noninteractive
ARG cryptsetup_repo=${cryptsetup_repo}
ARG cryptsetup_version=${cryptsetup_version}
ARG lvm2_repo=${lvm2_repo}
ARG lvm2_version=${lvm2_version}

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ENV TZ=UTC
RUN apt-get update && apt-get upgrade -y && \
    apt-get --no-install-recommends install -y \
	    apt-utils \
	    asciidoctor \
	    autoconf \
	    autopoint \
	    automake \
	    busybox-static \
	    ca-certificates \
	    curl \
	    gcc \
	    gettext \
	    git \
	    libaio-dev \
	    libblkid-dev \
	    libselinux1-dev \
	    libtool \
	    libpopt-dev \
	    libjson-c-dev \
	    libssl-dev \
	    make \
	    ninja-build \
	    pkg-config \
	    uuid-dev \
	    libseccomp-dev \
	    libseccomp2 \
	    zlib1g-dev &&\
    apt-get clean && rm -rf /var/lib/apt/lists/ && \
    build_root=$(mktemp -d) && \
    pushd ${build_root} && \
    echo "Build ${lvm2_repo} version: ${lvm2_version}" && \
    git clone --depth 1 --branch "${lvm2_version}" "${lvm2_repo}" lvm2 && \
    pushd lvm2 && \
    ./configure --enable-static_link --disable-selinux && \
    make && make install && \
    cp ./libdm/libdevmapper.pc /usr/lib/pkgconfig/devmapper.pc && \
    popd && \
    echo "Build ${cryptsetup_repo} version: ${cryptsetup_version}" && \
    git clone --depth 1 --branch "${cryptsetup_version}" "${cryptsetup_repo}" cryptsetup && \
    pushd cryptsetup && \
    ./autogen.sh && \
    ./configure  --enable-static --enable-static-cryptsetup --disable-udev --disable-external-tokens --disable-ssh-token && \
    make && make install && \
    strip /usr/sbin/veritysetup.static && \
    popd && \
    echo "Build gen_init_cpio tool" && \
    git clone --depth 1 --filter=blob:none --sparse https://github.com/torvalds/linux.git && \
    pushd linux && \
    git sparse-checkout add usr && cd usr && make gen_init_cpio && \
    install gen_init_cpio /usr/sbin/ && \
    popd && \
    popd && \
    rm -rf ${build_root}

COPY init.sh /usr/sbin/init.sh
