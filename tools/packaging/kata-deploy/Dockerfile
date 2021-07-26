# Copyright Intel Corporation.
#
# SPDX-License-Identifier: Apache-2.0

FROM jrei/systemd-centos
ARG KUBE_ARCH=amd64
ARG KATA_ARTIFACTS=./kata-static.tar.xz
ARG DESTINATION=/opt/kata-artifacts

COPY ${KATA_ARTIFACTS} .

RUN \
dnf install -y bzip2 jq && \
mkdir -p ${DESTINATION} && \
tar xvf ${KATA_ARTIFACTS} -C ${DESTINATION}/ && \
chown -R root:root ${DESTINATION}/

RUN \
curl -Lso /bin/kubectl https://storage.googleapis.com/kubernetes-release/release/$(curl -s https://storage.googleapis.com/kubernetes-release/release/stable.txt)/bin/linux/${KUBE_ARCH}/kubectl && \
chmod +x /bin/kubectl

COPY scripts ${DESTINATION}/scripts
RUN \
ln -s ${DESTINATION}/scripts/kata-deploy.sh /usr/bin/kata-deploy
