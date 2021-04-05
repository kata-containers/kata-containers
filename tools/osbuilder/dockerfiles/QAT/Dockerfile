# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Kata osbuilder 'works best' on Fedora
FROM fedora:latest

# Version of the Dockerfile - update if you change this file to avoid 'stale'
# images being pulled from the registry.
# Set AGENT_VERSION as an env variable to specify a specific version of Kata Agent to install

LABEL DOCKERFILE_VERSION="2.1"

ENV QAT_DRIVER_VER "QAT1.7.L.4.13.0-00009.tar.gz"
ENV QAT_DRIVER_URL "https://downloadmirror.intel.com/30178/eng/${QAT_DRIVER_VER}"
ENV QAT_CONFIGURE_OPTIONS "--enable-icp-sriov=guest"
ENV KATA_REPO_VERSION "main"
ENV AGENT_VERSION ""
ENV ROOTFS_OS "debian"
ENV OUTPUT_DIR "/output"

RUN dnf install -y \
    bc \
    bison \
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

# Add in non-privileged user
RUN useradd qatbuilder -p "" && \
    echo "qatbuilder ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers

# Pull in our local files
COPY ./run.sh /input/
COPY ./qat.conf /input/

# Change to a less privileged user before running the commands
USER qatbuilder

# Output is placed in the /output directory.
# We could make this a VOLUME to force it to be attached to the host, but let's
# just leave it as a container dir that can then be over-ridden from a host commandline
# volume setup.
# VOLUME /output

# By default build everything
CMD ["/input/run.sh"]
