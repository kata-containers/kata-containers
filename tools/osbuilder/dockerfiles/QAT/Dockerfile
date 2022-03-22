# Copyright (c) 2020-2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04

# Version of the Dockerfile - update if you change this file to avoid 'stale'
# images being pulled from the registry.
# Set AGENT_VERSION as an env variable to specify a specific version of Kata Agent to install

LABEL DOCKERFILE_VERSION="2.4"

ENV QAT_DRIVER_VER "QAT.L.4.15.0-00011.tar.gz"
ENV QAT_DRIVER_URL "https://downloadmirror.intel.com/649693/${QAT_DRIVER_VER}"
ENV QAT_CONFIGURE_OPTIONS "--enable-icp-sriov=guest"
ENV KATA_REPO_VERSION "main"
ENV AGENT_VERSION ""
ENV ROOTFS_OS "ubuntu"
ENV OUTPUT_DIR "/output"

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
    bc \
    bison \
    curl \
    debootstrap \
    diffutils \
    e2fsprogs \
    libelf-dev \
    libsystemd-dev \
    libudev-dev \
    findutils \
    flex \
    gcc \
    g++ \
    git \
    kmod \
    openssl \
    libssl-dev \
    make \
    multistrap \
    parted \
    patch \
    pkg-config \
    qemu-utils \
    sudo \
    xz-utils \
    yasm && \
    apt autoremove

# Add in non-privileged user
RUN useradd -r -G "sudo" -p "" -m qatbuilder && \
    chown -R qatbuilder:qatbuilder /home/qatbuilder && \
    echo "qatbuilder ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

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
