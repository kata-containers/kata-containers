#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

go_packages=.

candidates=`go list -f '{{.Dir}}/*.go' $go_packages`
for f in $candidates; do
	filename=`basename $f`
	# skip exit.go where, the only file we should call os.Exit() from.
	[[ $filename == "exit.go" ]] && continue
	# skip exit_test.go
	[[ $filename == "exit_test.go" ]] && continue
	# skip main_test.go
	[[ $filename == "main_test.go" ]] && continue
	files="$f $files"
done

if egrep -n '\<os\.Exit\>' $files; then
	echo "Direct calls to os.Exit() are forbidden, please use exit() so atexit() works"
	exit 1
fi
