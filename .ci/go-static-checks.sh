#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

# Perform static go tests.

function usage {
	echo "Usage $0 [OPTIONS] [PACKAGES]"
	echo "Perform static go checks on PACKAGES (./... by default)."
	echo
	echo "List of options:"
	echo "  -h, --help             print this help"
	echo "  -n, --no-network       do not access the network"
}

for i in "$@"; do
	case $i in
		-h|--help)
			usage
			exit 0
			;;
		-n|--no-network)
			NONETWORK=1
			shift
			;;
		*)
			args="$args $i"
			;;
	esac
done

go_packages=$args

[ -z "$go_packages" ] && {
	go_packages=$(go list ./... | grep -v vendor)
}

function install_package {
	url="$1"
	name=${url##*/}

	if [ -n "$NONETWORK" ]; then
		echo "Skipping updating package $name, no network access allowed"
		return
	fi

	echo Updating $name...
	go get -u $url
}

install_package github.com/fzipp/gocyclo
install_package github.com/client9/misspell/cmd/misspell
install_package github.com/golang/lint/golint
install_package github.com/gordonklaus/ineffassign
install_package github.com/opennota/check/cmd/structcheck
install_package honnef.co/go/tools/cmd/unused
install_package honnef.co/go/tools/cmd/staticcheck

echo Doing go static checks on packages: $go_packages

echo "Running misspell..."
go list -f '{{.Dir}}/*.go' $go_packages |\
    xargs -I % bash -c "misspell -error %"

echo "Running go vet..."
go vet $go_packages

cmd="gofmt -s -d -l"
echo "Running gofmt..."

# Note: ignore git directory in case any refs end in ".go" too.
diff=$(find . -not -wholename '*/vendor/*' -not -wholename '*/.git/*' -name '*.go' | \
	xargs $cmd)
if [ -n "$diff" -a $(echo "$diff" | wc -l) -ne 0 ]
then
	echo 2>&1 "ERROR: '$cmd' found problems:"
	echo 2>&1 "$diff"
	exit 1
fi

echo "Running cyclo..."
gocyclo -over 15 `go list -f '{{.Dir}}/*.go' $go_packages`

echo "Running golint..."
for p in $go_packages; do golint -set_exit_status $p; done

echo "Running ineffassign..."
go list -f '{{.Dir}}' $go_packages | xargs -L 1 ineffassign

for tool in structcheck unused staticcheck
do
	echo "Running ${tool}..."
	eval "$tool" "$go_packages"
done

echo "All Good!"
