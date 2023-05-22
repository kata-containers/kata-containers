# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04
ENV DEBIAN_FRONTEND=noninteractive
ENV INSTALL_IN_GOPATH=false

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
RUN if [ ${IMG_USER} != "root" ] && [ ! -z ${HOST_DOCKER_GID} ]; then groupadd --gid=${HOST_DOCKER_GID} docker_on_host;fi
RUN if [ ${IMG_USER} != "root" ] && [ ! -z ${HOST_DOCKER_GID} ]; then usermod -a -G docker_on_host ${IMG_USER};fi
RUN sh -c "echo '${IMG_USER} ALL=NOPASSWD: ALL' >> /etc/sudoers"

#FIXME: gcc is required as agent is build out of a container build.
RUN apt-get update && \
  apt-get install --no-install-recommends -y \
  build-essential \
  cpio \
  gcc \
  unzip \
  xz-utils && \
  if uname -m | grep -Eq 's390x'; then apt-get install -y s390-tools; fi && \
  apt-get clean && rm -rf /var/lib/apt/lists

ENV USER ${IMG_USER}
USER ${IMG_USER}
