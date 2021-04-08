#!/bin/bash
#
#Copyright (c) 2018 Intel Corporation
#
#SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out=""

handle_error() {
	echo "not ok"
	echo "output: ${out}"
}

OK() {
	echo "ok"
}
output_should_contain() {
	local output="$1"
	local text_to_find="$2"
	[ -n "$output" ]
	[ -n "$text_to_find" ]
	echo "${output}" | grep "${text_to_find}"
}

trap handle_error ERR

echo "Missing args show help"
out=$("${script_dir}/update-repository-version.sh" 2>&1) || (($? != 0))
echo "${out}" | grep Usage >>/dev/null
output_should_contain "${out}" "Usage"
OK

echo "Missing version show help"
out=$("${script_dir}/update-repository-version.sh" 2>&1) || (($? != 0))
echo "${out}" | grep Usage >>/dev/null
echo "${out}" | grep "no new version" >>/dev/null
OK

echo "help option"
out=$("${script_dir}/update-repository-version.sh" -h)
output_should_contain "${out}" "Usage"
OK

echo "Local update version update should work"
new_version="50.0.0-rc0"
out=$("${script_dir}/update-repository-version.sh" "${new_version}" "main" 2>&1)
output_should_contain "${out}" "release: Kata Containers ${new_version}"
OK
