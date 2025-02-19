# Copyright Intel Corporation, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

ARG BASE_IMAGE_NAME=alpine
ARG BASE_IMAGE_TAG=3.18
FROM $BASE_IMAGE_NAME:$BASE_IMAGE_TAG
ARG KATA_ARTIFACTS=./kata-static.tar.xz
ARG DESTINATION=/opt/kata-artifacts

COPY ${KATA_ARTIFACTS} ${WORKDIR}

# I understand that in order to be on the safer side, it'd
# be good to have the alpine packages pointing to a very
# specific version, but this may break anyone else trying
# to use a different version of alpine for one reason or
# another.  With this in mind, let's ignore DL3018.
# SC2086 is about using double quotes to prevent globbing and
# word splitting, which can also be ignored for now.
# hadolint ignore=DL3018,SC2086
RUN \
	apk --no-cache add bash curl && \
	ARCH=$(uname -m) && \
	if [ "${ARCH}" = "x86_64" ]; then ARCH=amd64; fi && \
	if [ "${ARCH}" = "aarch64" ]; then ARCH=arm64; fi && \
	DEBIAN_ARCH=${ARCH} && \
	if [ "${DEBIAN_ARCH}" = "ppc64le" ]; then DEBIAN_ARCH=ppc64el; fi && \
	curl -fL --progress-bar -o /usr/bin/kubectl https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/${ARCH}/kubectl && \
	chmod +x /usr/bin/kubectl && \
	curl -fL --progress-bar -o /usr/bin/jq https://github.com/jqlang/jq/releases/download/jq-1.7.1/jq-linux-${DEBIAN_ARCH} && \
	chmod +x /usr/bin/jq && \
	mkdir -p ${DESTINATION} && \
	tar xvf ${WORKDIR}/${KATA_ARTIFACTS} -C ${DESTINATION} && \
	rm -f ${WORKDIR}/${KATA_ARTIFACTS} && \
	apk del curl && \
	apk --no-cache add py3-pip && \
	pip install --no-cache-dir yq==3.2.3

COPY scripts ${DESTINATION}/scripts
COPY runtimeclasses ${DESTINATION}/runtimeclasses
