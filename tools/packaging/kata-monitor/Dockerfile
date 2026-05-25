# Copyright (c) 2020 Eric Ernst
# SPDX-License-Identifier: Apache-2.0
ARG GO_VERSION
FROM golang:${GO_VERSION}-alpine AS builder

RUN apk add --no-cache bash curl git make build-base
WORKDIR /go/src/github.com/kata-containers/kata-containers/src/runtime

COPY src/runtime/go.* .
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    go mod download

COPY . /go/src/github.com/kata-containers/kata-containers
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
	make SKIP_GO_VERSION_CHECK=true monitor

# run tests
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    cd pkg/kata-monitor/ && go test -c -o /tmp/tests
RUN /tmp/tests

FROM alpine:3.14
COPY --from=builder /go/src/github.com/kata-containers/kata-containers/src/runtime/kata-monitor /usr/bin/kata-monitor
CMD ["-h"]
ENTRYPOINT ["/usr/bin/kata-monitor"]
