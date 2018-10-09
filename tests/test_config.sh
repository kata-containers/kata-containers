#
# Copyright (c) 2018 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0

distrosSystemd=(fedora centos ubuntu debian suse)
distrosAgent=(alpine)

if [ $MACHINE_TYPE != "ppc64le" ]; then
	distrosSystemd+=(clearlinux)
fi

# "Not testing eurleros on Travis: (timeout, see: https://github.com/kata-containers/osbuilder/issues/46)"
if [ -z "${TRAVIS:-}" ]; then
	distrosSystemd+=(euleros)
fi

