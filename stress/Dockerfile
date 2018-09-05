#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

FROM golang:1.10.3-alpine3.8

MAINTAINER penny.zheng@arm.com

ENV GOPATH=/go
ENV PATH=$PATH:$GOPATH/bin

RUN apk add --no-cache musl-dev git gcc
RUN go get -u github.com/golang/dep/cmd/dep

RUN mkdir -p $GOPATH/src/stress/
WORKDIR $GOPATH/src/stress
RUN dep init
ADD main.go .
RUN dep ensure
RUN go build -o stress . && go install

ENTRYPOINT ["stress"]
