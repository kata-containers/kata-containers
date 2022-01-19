# Copyright (c) 2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

FROM ubuntu:20.04
ENV DEBIAN_FRONTEND=noninteractive
ENV INSTALL_IN_GOPATH=false

ARG http_proxy
ARG https_proxy

COPY install_yq.sh /usr/bin/install_yq.sh


# Install yq and docker
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        sudo && \
    apt-get clean && rm -rf /var/lib/apt/lists/ && \
    install_yq.sh && \
    curl -fsSL https://get.docker.com -o get-docker.sh && \
    sh get-docker.sh

ARG IMG_USER=kata-builder
ARG UID=1000
ARG GID=1000
RUN if [ ${IMG_USER} != "root" ]; then groupadd --gid=${GID} ${IMG_USER};fi
RUN if [ ${IMG_USER} != "root" ]; then adduser ${IMG_USER} --uid=${UID} --gid=${GID};fi
RUN sh -c "echo '${IMG_USER} ALL=NOPASSWD: ALL' >> /etc/sudoers"

#FIXME: gcc is required as agent is build out of a container build.
RUN apt-get update && \
  apt-get install --no-install-recommends -y \
  build-essential \
  cpio \
  gcc \
  git \
  make \
  xz-utils && \
  apt-get clean && rm -rf /var/lib/apt/lists

ENV USER ${IMG_USER}
USER ${UID}:${GID}

RUN mkdir ~/.docker/
RUN sh -c "echo ' \
{ \
 \"proxies\": \
 { \
   \"default\": \
   { \
     \"httpProxy\": \"${http_proxy}\", \
     \"httpsProxy\": \"${https_proxy}\", \
     \"noProxy\": \"localhost,127.0.0.1\" \
   } \
 } \
}' > ~/.docker/config.json"
