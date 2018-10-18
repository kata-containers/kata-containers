#
# Copyright (c) 2018 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0


if [ -n "${CI:-}" ]; then
	# "Not testing eurleros on Jenkins or Travis:
	# (unreliable mirros, see: https://github.com/kata-containers/osbuilder/issues/182)
	# (timeout, see: https://github.com/kata-containers/osbuilder/issues/46)"
	skipWhenTestingAll=(euleros)
fi

if [ -n "${TRAVIS:-}" ]; then
	skipWhenTestingAll+=()
fi

