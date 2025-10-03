# Copyright Intel Corporation, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

#### Nydus snapshotter & nydus image

FROM golang:1.24-alpine AS nydus-binary-downloader

# Keep the version here aligned with "ndyus-snapshotter.version"
# in versions.yaml
ARG NYDUS_SNAPSHOTTER_VERSION=v0.15.2
ARG NYDUS_SNAPSHOTTER_REPO=https://github.com/containerd/nydus-snapshotter

ARG DESTINATION=/opt/nydus-snapshotter/

RUN \
	mkdir -p ${DESTINATION} && \
	ARCH=$(uname -m) && \
	if [[ "${ARCH}" == "x86_64" ]]; then ARCH=amd64 ; fi && \
	if [[ "${ARCH}" == "aarch64" ]]; then ARCH=arm64; fi && \
	apk add --no-cache curl && \
	curl -fOL --progress-bar ${NYDUS_SNAPSHOTTER_REPO}/releases/download/${NYDUS_SNAPSHOTTER_VERSION}/nydus-snapshotter-${NYDUS_SNAPSHOTTER_VERSION}-linux-${ARCH}.tar.gz && \
	tar xvzpf nydus-snapshotter-${NYDUS_SNAPSHOTTER_VERSION}-linux-${ARCH}.tar.gz -C ${DESTINATION} && \
	rm nydus-snapshotter-${NYDUS_SNAPSHOTTER_VERSION}-linux-${ARCH}.tar.gz

#### kata-deploy main image

FROM quay.io/kata-containers/kata-deploy-ci:kata-containers-latest-amd64
ARG DESTINATION=/opt/kata-artifacts

# I understand that in order to be on the safer side, it'd
# be good to have the alpine packages pointing to a very
# specific version, but this may break anyone else trying
# to use a different version of alpine for one reason or
# another.  With this in mind, let's ignore DL3018.
# SC2086 is about using double quotes to prevent globbing and
# word splitting, which can also be ignored for now.
# hadolint ignore=DL3018,SC2086
RUN \
	mkdir -p ${DESTINATION}

COPY scripts ${DESTINATION}/scripts
COPY nydus-snapshotter ${DESTINATION}/nydus-snapshotter
COPY --from=nydus-binary-downloader /opt/nydus-snapshotter/bin/containerd-nydus-grpc ${DESTINATION}/nydus-snapshotter/
COPY --from=nydus-binary-downloader /opt/nydus-snapshotter/bin/nydus-overlayfs ${DESTINATION}/nydus-snapshotter/
