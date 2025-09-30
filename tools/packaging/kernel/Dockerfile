FROM registry.ddbuild.io/images/base/gbi-ubuntu_2204:release

ARG GOLANG_VERSION=1.22.2
ARG TARGETARCH

USER root

# Install the required packages
RUN apt-get update && apt-get install -y \
    flex \
    bison \
    libelf-dev \
    libssl-dev \
    pahole \
    xz-utils \
    gcc \
    make \
    patch \
    bc \
    python3-pip

# Install golang
RUN curl -fs https://dl.google.com/go/go$GOLANG_VERSION.linux-${TARGETARCH}.tar.gz | tar -v -C /usr/local -xz
ENV GOPATH /go
ENV PATH $GOPATH/bin:/usr/local/go/bin:$PATH

# Install yq

RUN go install github.com/mikefarah/yq/v4@latest

# Install AWS CLI

RUN pip3 install awscli
