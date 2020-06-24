#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

init_git_credential_cache() {
	#This is needed to setup github credentials to do push  in a job
	(

		set -o errexit
		set -o nounset
		set -o pipefail
		set -o errtrace
		set +x

		readonly token_sh=$(mktemp)
		readonly agent_clone=$(mktemp -d)
		finish() {
			rm -rf "${token_sh}"
			rm -rf "${agent_clone}"
		}
		trap finish EXIT

		chmod 700 "${token_sh}"
		cat <<EOT >"${token_sh}"
#!/bin/bash
echo "\$GITHUB_TOKEN"
EOT
		export GIT_ASKPASS=${token_sh}

		#cache credential
		git config --global credential.helper cache
		#setup credential
		git clone https://github.com/katabuilder/agent.git "${agent_clone}"
		cd "${agent_clone}" || exit 1
		#this set the credential for first time
		git push
		# not needed anymore
		unset GIT_ASKPASS
	) >>/dev/null
}

main() {
	[ -n "$GITHUB_TOKEN" ] || die "GITHUB_TOKEN not set"
	init_git_credential_cache
}

main $@
