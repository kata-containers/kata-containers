#!/usr/bin/env bats
# *-*- Mode: sh; sh-basic-offset: 8; indent-tabs-mode: nil -*-*
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Docker compose version
VERSION="1.20.0"
# URL
URL="http://localhost:5000"
# Image name
IMAGE_NAME="dockercompose_web"
# Installation path
INSTALLATION_PATH="/usr/local/bin/docker-compose"

setup () {
	skip "This is not working (https://github.com/kata-containers/runtime/issues/175)"
	run command -v docker-compose
	# Check that Docker compose is installed in the system
	if [ "$status" -ne 0 ]; then
		echo >&2 "ERROR: Docker compose should be installed in the system."
		# Install docker-compose
		echo >&2 "Installing Docker compose."
		curl -L https://github.com/docker/compose/releases/download/${VERSION}/docker-compose-`uname -s`-`uname -m` -o ${INSTALLATION_PATH}
		sudo -E chmod +x ${INSTALLATION_PATH}
	fi

	# Run the services in the background
	docker-compose up -d
}

@test "run REDIS application" {
	skip "This is not working (https://github.com/clearcontainers/runtime/issues/1042)"
	# Check that services are running
	CHECK_CONTAINERS=$(docker-compose ps -q)
	if [[ -z ${CHECK_CONTAINERS} ]]; then
		echo >&2 "ERROR: Containers are not running."
		exit 1
	fi

	# Check that REDIS application is running
	curl -s ${URL} | grep "Hello World!"
}

teardown() {
	skip "This is not working (https://github.com/clearcontainers/runtime/issues/1042)"
	#Stop containers
	docker-compose stop

	# Remove containers
	docker-compose rm -f

	# Remove volume
	docker-compose down --volumes

	# Remove image
	docker rmi ${IMAGE_NAME}
}
