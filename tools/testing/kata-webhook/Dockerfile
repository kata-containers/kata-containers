# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
ARG IMAGE_REGISTRY=docker.io
FROM ${IMAGE_REGISTRY}/golang:1.21.5-alpine3.19 AS builder

WORKDIR /go/src/kata-pod-annotate

COPY . ./
RUN CGO_ENABLED=0 go build -o /go/bin/kata-pod-annotate

FROM ${IMAGE_REGISTRY}/alpine:3.19
COPY --from=builder /go/bin/kata-pod-annotate /kata-pod-annotate
ENTRYPOINT ["/kata-pod-annotate"]

