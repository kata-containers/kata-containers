# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
ENV INSTALL_IN_GOPATH=false
# Required for libxml2-dev
ENV TZ=Etc/UTC
ARG ARCH

COPY install_yq.sh /usr/bin/install_yq.sh
COPY install_oras.sh /usr/bin/install_oras.sh

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Install yq, oras, and docker
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        make \
        git \
        wget \
        sudo && \
    apt-get clean && rm -rf /var/lib/apt/lists/ && \
    install_yq.sh && \
    install_oras.sh && \
    curl -fsSL https://get.docker.com -o get-docker.sh && \
    if uname -m | grep -Eq 's390x|ppc64le'; then export VERSION="v20.10" && \
    sed -i 's/\<docker-compose-plugin\>//g' get-docker.sh; fi && \
    sh get-docker.sh

ARG IMG_USER=kata-builder
ARG UID=1000
ARG GID=1000
# gid of the docker group on the host, required for running docker in docker builds.
ARG HOST_DOCKER_GID

RUN if [ ${IMG_USER} != "root" ]; then sed -i -e "/:${GID}:/d" /etc/group; groupadd --gid=${GID} ${IMG_USER};fi
RUN if [ ${IMG_USER} != "root" ]; then adduser ${IMG_USER} --uid=${UID} --gid=${GID};fi
RUN if [ ${IMG_USER} != "root" ] && [ ! -z ${HOST_DOCKER_GID} ] && [ ! $(getent group | grep :${HOST_DOCKER_GID}:) ]; then groupadd --gid=${HOST_DOCKER_GID} docker_on_host;fi
RUN if [ ${IMG_USER} != "root" ] && [ ! -z ${HOST_DOCKER_GID} ]; then usermod -a -G ${HOST_DOCKER_GID} ${IMG_USER};fi
RUN sh -c "echo '${IMG_USER} ALL=NOPASSWD: ALL' >> /etc/sudoers"

RUN if [ "${ARCH}" != "$(uname -m)" ] && [ "${ARCH}" == "s390x" ]; then sed -i 's/^deb/deb [arch=amd64]/g' /etc/apt/sources.list && \
    dpkg --add-architecture "s390x" && \
    echo "deb [arch=s390x] http://ports.ubuntu.com/ jammy main multiverse universe" >> /etc/apt/sources.list && \
    echo "deb [arch=s390x] http://ports.ubuntu.com/ jammy-security main multiverse universe" >> /etc/apt/sources.list && \
    echo "deb [arch=s390x] http://ports.ubuntu.com/ jammy-backports main multiverse universe" >> /etc/apt/sources.list && \
    echo "deb [arch=s390x] http://ports.ubuntu.com/ jammy-updates main multiverse universe" >> /etc/apt/sources.list; fi

#FIXME: gcc is required as agent is build out of a container build.
RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    build-essential \
    cpio \
    gcc \
    unzip \
    git \
    make \
    wget \
    xz-utils && \
    if [ "${ARCH}" != "$(uname -m)" ] && [ "${ARCH}" == "s390x" ]; then \
        apt-get install -y --no-install-recommends \
        gcc-s390x-linux-gnu \
        g++-s390x-linux-gnu \
        binutils-s390x-linux-gnu \
        dpkg-dev \
        apt-utils \
        libssl-dev:s390x \
        libcurl4-openssl-dev:s390x \
        libjson-c-dev:s390x \
        pkg-config:s390x \
        libxml2-dev:s390x \
        libjson-c-dev:s390x \
        libglib2.0-0:s390x \
        libglib2.0-dev:s390x; \
    elif uname -m | grep -Eq 's390x'; then apt-get install -y s390-tools; fi && \
    apt-get clean && rm -rf /var/lib/apt/lists

RUN if [ "${ARCH}" != "$(uname -m)" ] && [ "${ARCH}" == "s390x" ]; then \
        git clone -b v2.25.0 https://github.com/ibm-s390-linux/s390-tools.git && cd s390-tools && \
        pushd genprotimg && pushd boot && make CROSS_COMPILE=s390x-linux-gnu- && popd && pushd src && \
        make CROSS_COMPILE=s390x-linux-gnu- && popd && make install && popd || return; fi

ENV USER ${IMG_USER}
USER ${IMG_USER}
