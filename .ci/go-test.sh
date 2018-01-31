## Copyright 2018 99Cloud Inc.
##
## SPDX-License-Identifier: Apache-2.0
##

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

run_go_test
