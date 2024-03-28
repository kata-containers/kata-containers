# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
FROM golang:1.16-alpine3.14 AS builder

WORKDIR /go/src/kata-pod-annotate

COPY . ./
RUN CGO_ENABLED=0 go build -o /go/bin/kata-pod-annotate

FROM alpine:3.14
COPY --from=builder /go/bin/kata-pod-annotate /kata-pod-annotate
ENTRYPOINT ["/kata-pod-annotate"]

