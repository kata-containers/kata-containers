# Copyright Intel Corporation.
#
# SPDX-License-Identifier: Apache-2.0

FROM registry.access.redhat.com/ubi8/ubi-minimal:latest AS base

RUN microdnf update -y && \
microdnf clean all

RUN microdnf -y install systemd && \
microdnf clean all && \
(cd /lib/systemd/system/sysinit.target.wants/; for i in *; do [ $i == systemd-tmpfiles-setup.service ] || rm -f $i; done); \
rm -f /lib/systemd/system/multi-user.target.wants/*;\
rm -f /etc/systemd/system/*.wants/*;\
rm -f /lib/systemd/system/local-fs.target.wants/*; \
rm -f /lib/systemd/system/sockets.target.wants/*udev*; \
rm -f /lib/systemd/system/sockets.target.wants/*initctl*; \
rm -f /lib/systemd/system/basic.target.wants/*;\
rm -f /lib/systemd/system/anaconda.target.wants/*;

VOLUME [ "/sys/fs/cgroup" ]
CMD [ "/usr/sbin/init" ]

FROM base

ARG KUBE_ARCH=amd64
ARG KATA_ARTIFACTS=./kata-static.tar.xz
ARG DESTINATION=/opt/kata-artifacts

COPY ${KATA_ARTIFACTS} .

RUN \
microdnf install -y bzip2 dbus jq tar xz && \
mkdir -p ${DESTINATION} && \
tar xvf ${KATA_ARTIFACTS} -C ${DESTINATION}/ && \
chown -R root:root ${DESTINATION}/

RUN \
curl -Lso /usr/bin/kubectl https://storage.googleapis.com/kubernetes-release/release/$(curl -s https://storage.googleapis.com/kubernetes-release/release/stable.txt)/bin/linux/${KUBE_ARCH}/kubectl && \
chmod +x /usr/bin/kubectl

COPY scripts ${DESTINATION}/scripts
RUN \
ln -s ${DESTINATION}/scripts/kata-deploy.sh /usr/bin/kata-deploy
