# Copyright (c) 2026 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Minimal image for the job-mode dispatcher (kata-deploy-job-dispatcher).
#
# Unlike the kata-deploy image, this dispatcher never touches the host: it only
# talks to the Kubernetes API (lists nodes, creates/watches per-node Jobs). It
# therefore needs nothing but the statically-linked binary and CA certificates,
# and ships on distroless/static.
#
# The binary is produced by the shared rust-builder stage and packaged into
# kata-deploy-static-kata-deploy-job-dispatcher.tar.zst (see Dockerfile.components and
# local-build/kata-deploy-build-components-tarballs.sh). Build from the repo
# root so the tarball path resolves:
#   docker build -f tools/packaging/kata-deploy/job-dispatcher/Dockerfile .

#### Extract the dispatcher binary from its tarball
FROM alpine:3.22 AS extract-stage

ARG KATA_ARTIFACTS_DIR=tools/packaging/kata-deploy/kata-artifacts

SHELL ["/bin/ash", "-eo", "pipefail", "-c"]

RUN apk add --no-cache zstd

COPY ${KATA_ARTIFACTS_DIR}/kata-deploy-static-kata-deploy-job-dispatcher.tar.zst /tmp/dispatcher.tar.zst

RUN \
	mkdir -p /opt/dispatcher && \
	zstd -dc /tmp/dispatcher.tar.zst | tar -xf - -C /opt/dispatcher ./usr/bin/kata-deploy-job-dispatcher

#### Dispatcher image
# distroless does not publish pinned/versioned tags - only rolling ones
# (latest, nonroot, debug) - so :latest is the intended way to consume it.
# hadolint ignore=DL3007
FROM gcr.io/distroless/static-debian13:latest

COPY --from=extract-stage /opt/dispatcher/usr/bin/kata-deploy-job-dispatcher /usr/bin/kata-deploy-job-dispatcher

ENTRYPOINT ["/usr/bin/kata-deploy-job-dispatcher"]
