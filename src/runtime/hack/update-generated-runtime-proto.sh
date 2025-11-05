#!/usr/bin/env bash
#
# Copyright 2019 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit -o pipefail -o nounset

# Define the root directory for all proto files
BASEDIR="$(dirname "$0")"
cd ${BASEDIR}/..
BASEPATH=`pwd`

declare -A proto_files_dict
proto_files_dict['protocols/cdiresolver/cdiresolver.proto']="ttrpc"
proto_files_dict['protocols/cache/cache.proto']="grpc"

for f in "${!proto_files_dict[@]}"; do
	echo -e "\n   [golang] compiling ${f} ..."
	PROTOPATH=$(dirname ${f})
	PROTO="${proto_files_dict[${f}]}"
	protoc \
		-I="${PROTOPATH}":"${BASEPATH}/vendor" \
		--go_out=paths=source_relative:${PROTOPATH} \
		--go-${PROTO}_out=paths=source_relative:${PROTOPATH} \
		${f}
	echo -e "   [golang] ${f} compiled\n"
done
