# Copyright (c) 2020 Eric Ernst
# SPDX-License-Identifier: Apache-2.0

FROM golang:1.25-alpine AS builder

RUN apk add --no-cache bash curl git make build-base linux-headers
WORKDIR /go/src/github.com/kata-containers/kata-containers/src/runtime
COPY . /go/src/github.com/kata-containers/kata-containers
RUN make SKIP_GO_VERSION_CHECK=true monitor

FROM alpine:3.20
COPY --from=builder /go/src/github.com/kata-containers/kata-containers/src/runtime/kata-monitor /usr/bin/kata-monitor
ENTRYPOINT ["/usr/bin/kata-monitor"]
