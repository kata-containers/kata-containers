#!/bin/bash

set -o errexit
set -o pipefail
set -o nounset

die() {
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

# Entrypoint for the container image, we know that the AKS and Kata setup/testing
# scripts are located at root.

cd obs-packaging
bash -x ./gen_versions_txt.sh ${BRANCH}
cd ../release
bash -x ./publish-kata-image.sh -p ${NEW_VERSION}
bash -x ./kata-deploy-binaries.sh -p ${NEW_VERSION}

echo "maybe it worked"
