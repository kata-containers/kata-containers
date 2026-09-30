# check=skip=InvalidDefaultArgInFrom
# InvalidDefaultArgInFrom: Suppresses "Default value for ARG golang:${GO_VERSION}-alpine results in empty or invalid base image name" as GO_VERSION is supplied from Makefile
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

ARG GO_VERSION=

FROM golang:${GO_VERSION}-alpine AS builder

WORKDIR /go/src/kata-pod-annotate

COPY . ./
RUN CGO_ENABLED=0 go build -o /go/bin/kata-pod-annotate

FROM alpine:3.19
COPY --from=builder /go/bin/kata-pod-annotate /kata-pod-annotate
ENTRYPOINT ["/kata-pod-annotate"]

