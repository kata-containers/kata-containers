#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Check there are no os.Exit() calls creeping into the code
# We don't use that exit path in the Kata codebase.

# Allow the path to check to be over-ridden.
# Default to the current directory.
go_packages=${1:-.}

echo "Checking for no os.Exit() calls for package [${go_packages}]"

candidates=`go list -f '{{.Dir}}/*.go' $go_packages`
for f in $candidates; do
	filename=`basename $f`
	# skip all go test files
	[[ $filename == *_test.go ]] && continue
	# skip exit.go where, the only file we should call os.Exit() from.
	[[ $filename == "exit.go" ]] && continue
	files="$f $files"
done

[ -z "$files" ] && echo "No files to check, skipping" && exit 0

if egrep -n '\<os\.Exit\>' $files; then
	echo "Direct calls to os.Exit() are forbidden, please use exit() so atexit() works"
	exit 1
fi
