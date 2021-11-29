# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04
ENV DEBIAN_FRONTEND=noninteractive
ENV INSTALL_IN_GOPATH=false

ADD install_yq.sh /usr/bin/install_yq.sh


# yq installer deps
RUN apt update && apt-get install -y curl sudo

# Install yq
RUN install_yq.sh

RUN curl -fsSL https://get.docker.com -o get-docker.sh
RUN sh get-docker.sh

ARG IMG_USER=kata-builder
ARG UID=1000
ARG GID=1000
RUN if [ ${IMG_USER} != "root" ]; then groupadd --gid=${GID} ${IMG_USER};fi
RUN if [ ${IMG_USER} != "root" ]; then adduser ${IMG_USER} --uid=${UID} --gid=${GID};fi
RUN sh -c "echo '${IMG_USER} ALL=NOPASSWD: ALL' >> /etc/sudoers"

#FIXME: gcc is required as agent is build out of a container build.
RUN apt-get update && \
    apt install --no-install-recommends -y \
    cpio \
    gcc \
    git \
    make \
    xz-utils

ENV USER ${IMG_USER}
USER ${UID}:${GID}
