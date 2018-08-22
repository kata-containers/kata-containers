# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Set up an Ubuntu image with the components needed to run `fio`
FROM ubuntu

# Version of the Dockerfile
LABEL DOCKERFILE_VERSION="1.0"

# Without this some of the package installs stop to try and ask questions...
#ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
	apt-get install -y fio && \
	apt-get remove -y unattended-upgrades

# Pull in our actual worker scripts
COPY . /scripts

# By default generate the report
CMD ["/scripts/init.sh"]
