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
	script -qfc $@;
}

faketty $@
