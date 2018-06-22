#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

# function to run unit test always with a tty
function faketty()
{
	if [[ "$TRAVIS_OS_NAME" == "linux" ]]; then script -qfec $@; fi
	if [[ "$TRAVIS_OS_NAME" == "osx" ]]; then $@; fi
}

faketty $@
