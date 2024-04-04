#!/bin/bash
# Copyright (c) 2024 Red Hat, Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
if [ "$#" -gt 2 ] || [ "$#" -lt 1 ] ; then
	echo "Usage: $0 GOOD [BAD]"
	echo "Prints list of available kata-deploy-ci tags between GOOD and BAD commits (by default BAD is the latest available tag)"
	exit 255
fi
GOOD="$1"
[ -n "$2" ] && BAD="$2"
ARCH=amd64
REPO="quay.io/kata-containers/kata-deploy-ci"

TAGS=$(skopeo list-tags "docker://$REPO")
# Only amd64
TAGS=$(echo "$TAGS" | jq '.Tags' | jq "map(select(endswith(\"$ARCH\")))" | jq -r '.[]')
# Tags since $GOOD
TAGS=$(echo "$TAGS" | sed -n -e "/$GOOD/,$$p")
# Tags up to $BAD
[ -n "$BAD" ] && TAGS=$(echo "$TAGS" | sed "/$BAD/q")
# Comma separated tags with repo
echo "$TAGS" | sed -e "s@^@$REPO:@" | paste -s -d, -
