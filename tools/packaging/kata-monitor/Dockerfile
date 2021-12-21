# Copyright (c) 2020 Eric Ernst
# SPDX-License-Identifier: Apache-2.0

FROM golang:1.15-alpine AS builder

RUN apk add --no-cache bash curl git make
WORKDIR /go/src/github.com/kata-containers/kata-containers/src/runtime
COPY . /go/src/github.com/kata-containers/kata-containers
RUN SKIP_GO_VERSION_CHECK=true make monitor

FROM alpine:3.14
COPY --from=builder /go/src/github.com/kata-containers/kata-containers/src/runtime/kata-monitor /usr/bin/kata-monitor
CMD ["-h"]
ENTRYPOINT ["/usr/bin/kata-monitor"]
