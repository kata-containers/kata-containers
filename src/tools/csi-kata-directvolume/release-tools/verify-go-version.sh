#!/usr/bin/env bash
#
# Copyright 2019 The Kubernetes Authors.
#
# SPDX-License-Identifier: Apache-2.0
#

GO="$1"

if [ ! "$GO" ]; then
    echo >&2 "usage: $0 <path to go binary>"
    exit 1
fi

die () {
    echo "ERROR: $*"
    exit 1
}

version=$("$GO" version) || die "determining version of $GO failed"
# shellcheck disable=SC2001
majorminor=$(echo "$version" | sed -e 's/.*go\([0-9]*\)\.\([0-9]*\).*/\1.\2/')

if [ "$majorminor" != "$expected" ]; then
    cat >&2 <<EOF

======================================================
                  WARNING

  Compile the Project with Go version v$majorminor !

======================================================

EOF
fi
