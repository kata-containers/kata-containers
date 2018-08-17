#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Setup the environment and build the snap image.
# This script runs in the VM.

set -x -e

sudo apt-get update -y
sudo apt-get install -y \
	build-essential \
	cpio \
	docker.io \
	golang-go \
	libattr1-dev \
	libcap-dev \
	libcap-ng-dev \
	libdw-dev \
	libelf-dev \
	libfdt-dev \
	libglib2.0-dev \
	libiberty-dev \
	libnewt-dev \
	libpci-dev \
	libpixman-1-dev \
	librbd-dev \
	libssl-dev \
	libz-dev \
	openssl \
	python \
	snapcraft \
	snapd

# start docker
sudo systemctl start docker

# clone packaging reposiory and make snap
packaging_repo_url=https://github.com/kata-containers/packaging
packaging_dir=~/packaging
sudo rm -rf ${packaging_dir}
git clone ${packaging_repo_url} ${packaging_dir}
pushd ${packaging_dir}
sudo -E PATH=$PATH make snap
sudo chown ${USER}:${USER} *.snap
