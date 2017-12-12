#
# Copyright 2017 Huawei Corporation.
#
# SPDX-License-Identifier: Apache-2.0
#
# This file is used for building and testing agent.
# It includes some useful tools for development of agent
# which can ensure everyone using similar development toolkit

FROM centos:7

ARG http_proxy
ARG https_proxy

# TODO: if you have trouble downloading git repos due to cert problem
# try to uncomment this
# ENV GIT_SSL_NO_VERIFY true

# install building tools
RUN yum makecache && yum install -y \
	git automake libtool glibc-headers gcc-c++ make

# install GO development environment
ENV GO_VERSION 1.9.2
RUN curl -fkL https://storage.googleapis.com/golang/go${GO_VERSION}.linux-amd64.tar.gz \
	| tar -zxC /usr/local/
ENV GOPATH /go
ENV PATH $PATH:/go/bin:/usr/local/go/bin

# install golang/protobuf
ENV PROTOBUF_PROTOC_COMMIT 1e59b77b52bf8e4b449a57e6f79f21226d571845
ENV PROTOBUF_VERSION 3.5.0
RUN curl -fkL https://github.com/google/protobuf/releases/download/v${PROTOBUF_VERSION}/protobuf-cpp-${PROTOBUF_VERSION}.tar.gz \
	| tar -zxC /opt && cd /opt/protobuf-${PROTOBUF_VERSION} \
	&& ./autogen.sh && ./configure && make && make install
RUN go get -d github.com/golang/protobuf/protoc-gen-go \
	&& cd $GOPATH/src/github.com/golang/protobuf/ \
	&& git checkout -q ${PROTOBUF_PROTOC_COMMIT} \
	&& go install github.com/golang/protobuf/protoc-gen-go

# install gogo/protobuf
ENV GOGO_COMMIT 41168f6614b7bb144818ec8967b8c702705df564
RUN go get -d -v github.com/gogo/protobuf/{proto,jsonpb,protoc-gen-gogo,gogoproto}
RUN cd $GOPATH/src/github.com/gogo/protobuf && git checkout -q ${GOGO_COMMIT} \
	&& go install github.com/gogo/protobuf/{proto,jsonpb,protoc-gen-gogo,gogoproto}

# add agent repository
ADD . ${GOPATH}/src/github.com/kata-containers/agent

# default working dir should be agent dir
WORKDIR ${GOPATH}/src/github.com/kata-containers/agent

ENTRYPOINT ["bash", "-c"]

CMD ["/bin/bash"]
