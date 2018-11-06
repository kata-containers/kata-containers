#
# Copyright (c) 2018 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0

# List of distros not to test, when running all tests with test_images.sh
typeset -a skipWhenTestingAll

if [ -n "${TRAVIS:-}" ]; then
	# (travis may timeout with euleros, see:
	#  https://github.com/kata-containers/osbuilder/issues/46)"
	skipWhenTestingAll+=(euleros)
fi

