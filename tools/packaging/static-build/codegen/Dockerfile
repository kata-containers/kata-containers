# Copyright (c) 2025 Edgeless Systems gmbH
#
# SPDX-License-Identifier: Apache-2.0

ARG GO_VERSION=

FROM busybox:1.37.0 AS resources

WORKDIR /usr/local

ARG PROTOC_VERSION=
RUN wget -O /tmp/protoc.zip https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip && \
    unzip /tmp/protoc.zip

FROM golang:${GO_VERSION}

COPY --from=resources /usr/local /usr

ARG PROTOC_GEN_GO_VERSION=
RUN go install google.golang.org/protobuf/cmd/protoc-gen-go@${PROTOC_GEN_GO_VERSION}
ARG TTRPC_VERSION=
RUN go install github.com/containerd/ttrpc/cmd/protoc-gen-go-ttrpc@${TTRPC_VERSION}
RUN go install github.com/containerd/ttrpc/cmd/protoc-gen-gogottrpc@${TTRPC_VERSION}
