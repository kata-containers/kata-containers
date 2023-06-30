# Copyright Intel Corporation, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

# Specify alternative base image, e.g. clefos for s390x
ARG BASE_IMAGE_NAME=ubuntu
ARG BASE_IMAGE_TAG=20.04
FROM $BASE_IMAGE_NAME:$BASE_IMAGE_TAG
ENV DEBIAN_FRONTEND=noninteractive
ARG KATA_ARTIFACTS=./kata-static.tar.xz
ARG DESTINATION=/opt/kata-artifacts

COPY ${KATA_ARTIFACTS} ${WORKDIR}

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN \
apt-get update && \
apt-get install -y --no-install-recommends apt-transport-https ca-certificates curl gpg xz-utils systemd && \
mkdir -p /etc/apt/keyrings/ && \
curl -fsSL https://packages.cloud.google.com/apt/doc/apt-key.gpg | gpg --dearmor -o /etc/apt/keyrings/kubernetes-archive-keyring.gpg && \
echo "deb [signed-by=/etc/apt/keyrings/kubernetes-archive-keyring.gpg] https://apt.kubernetes.io/ kubernetes-xenial main" | tee /etc/apt/sources.list.d/kubernetes.list && \
apt-get update && \
apt-get install -y --no-install-recommends kubectl && \
apt-get clean && rm -rf /var/lib/apt/lists/ && \
mkdir -p ${DESTINATION} && \
tar xvf ${WORKDIR}/${KATA_ARTIFACTS} -C ${DESTINATION} && \
rm -f ${WORKDIR}/${KATA_ARTIFACTS}

COPY scripts ${DESTINATION}/scripts
