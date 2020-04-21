#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# The main purpose of this test is to print
# the entropy level inside the container after
# we installed the haveged package. We need to
# verify that the entropy level is not < 1000
# https://wiki.archlinux.org/index.php/Haveged

source /etc/os-release || source /usr/lib/os-release
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

# Environment variables
IMAGE="busybox"
# This the minimum entropy level produced
# by haveged is 1000 see https://wiki.archlinux.org/index.php/Haveged
# Less than 1000 could potentially slow down cryptographic
# applications see https://www.suse.com/support/kb/doc/?id=7011351
ENTROPY_LEVEL="1000"

setup() {
	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]

	# Check if haveged package is installed
	check_package=$(which haveged | wc -l)

	# Install haveged package if is not installed
	if [ $check_package -eq 0 ]; then
		case "$ID" in
			ubuntu )
				sudo -E apt install -y haveged
				;;
			fedora )
				sudo -E dnf -y install haveged
				;;
			centos )
				sudo -E yum install -y haveged
				;;
			opensuse-* | sled | sles )
				sudo -E zypper install -y haveged
				;;
		esac
		sudo systemctl start haveged
	fi
}

@test "check entropy level" {
	run docker run --rm --runtime=${RUNTIME} ${IMAGE} sh -c "cat /proc/sys/kernel/random/entropy_avail"
	echo "$output"
	[ "$output" -ge ${ENTROPY_LEVEL} ]
}

teardown() {
	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
