# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Kata osbuilder 'works best' on Fedora
FROM fedora:latest

# Version of the Dockerfile - update if you change this file to avoid 'stale'
# images being pulled from the registry.
# Set AGENT_VERSION as an env variable to specify a specific version of Kata Agent to install

LABEL DOCKERFILE_VERSION="2.0"

ENV QAT_DRIVER_VER "qat1.7.l.4.12.0-00011.tar.gz"
ENV QAT_DRIVER_URL "https://downloadmirror.intel.com/30178/eng/${QAT_DRIVER_VER}"
ENV QAT_CONFIGURE_OPTIONS "--enable-icp-sriov=guest"
ENV KATA_REPO_VERSION "main"
ENV AGENT_VERSION ""
ENV ROOTFS_OS "debian"
ENV OUTPUT_DIR "/output"

RUN dnf install -y \
    bc \
    bison \
    curl \
    debootstrap \
    diffutils \
    e2fsprogs \
    elfutils-libelf-devel \
    findutils \
    flex \
    gcc \
    gcc-c++ \
    git \
    kiwi \
    kmod \
    openssl \
    openssl-devel \
    make \
    parted \
    patch \
    qemu-img \
    systemd-devel \
    sudo \
    xz

# Pull in our local files
COPY ./run.sh /input/
COPY ./qat.conf /input/

# Output is placed in the /output directory.
# We could make this a VOLUME to force it to be attached to the host, but let's
# just leave it as a container dir that can then be over-ridden from a host commandline
# volume setup.
# VOLUME /output

# By default build everything
CMD ["/input/run.sh"]
