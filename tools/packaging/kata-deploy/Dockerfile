# Copyright Intel Corporation, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

# Specify alternative base image, e.g. clefos for s390x
ARG IMAGE
FROM ${IMAGE:-registry.centos.org/centos}:7
ARG KATA_ARTIFACTS=./kata-static.tar.xz
ARG DESTINATION=/opt/kata-artifacts

COPY ${KATA_ARTIFACTS} ${WORKDIR}

RUN \
yum -y update && \
yum -y install xz && \
yum clean all && \
mkdir -p ${DESTINATION} && \
tar xvf ${KATA_ARTIFACTS} -C ${DESTINATION}

# hadolint will deny echo -e, heredocs don't work in Dockerfiles, shell substitution doesn't work with $'...'
RUN \
echo "[kubernetes]" >> /etc/yum.repos.d/kubernetes.repo && \
echo "name=Kubernetes" >> /etc/yum.repos.d/kubernetes.repo && \
echo "baseurl=https://packages.cloud.google.com/yum/repos/kubernetes-el7-$(uname -m)" >> /etc/yum.repos.d/kubernetes.repo && \
echo "gpgkey=https://packages.cloud.google.com/yum/doc/yum-key.gpg https://packages.cloud.google.com/yum/doc/rpm-package-key.gpg" >> /etc/yum.repos.d/kubernetes.repo && \
yum -y install kubectl && \
yum clean all

COPY scripts ${DESTINATION}/scripts
