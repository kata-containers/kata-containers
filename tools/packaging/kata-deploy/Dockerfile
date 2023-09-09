# Copyright Intel Corporation, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

ARG BASE_IMAGE_NAME=alpine
ARG BASE_IMAGE_TAG=3.18
FROM $BASE_IMAGE_NAME:$BASE_IMAGE_TAG
ARG KATA_ARTIFACTS=./kata-static.tar.xz
ARG DESTINATION=/opt/kata-artifacts

COPY ${KATA_ARTIFACTS} ${WORKDIR}

RUN \
	apk --no-cache add bash curl && \
	ARCH=$(uname -m) && \
	if [ "${ARCH}" = "x86_64" ]; then ARCH=amd64; fi && \
	if [ "${ARCH}" = "aarch64" ]; then ARCH=arm64; fi && \
	curl -fL --progress-bar -o /usr/bin/kubectl https://storage.googleapis.com/kubernetes-release/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/${ARCH}/kubectl && \
	chmod +x /usr/bin/kubectl && \
	mkdir -p ${DESTINATION} && \
	tar xvf ${WORKDIR}/${KATA_ARTIFACTS} -C ${DESTINATION} && \
	rm -f ${WORKDIR}/${KATA_ARTIFACTS}

COPY scripts ${DESTINATION}/scripts
COPY runtimeclasses ${DESTINATION}/runtimeclasses
