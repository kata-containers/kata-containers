#!/usr/bin/env bash

# Copyright 2026 Google LLC
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Purge every Debian package the guest does not need, in the rootfs stage of
# Dockerfile, after the guest's packages are installed and while the apt lists are
# still present.
#
# Usage: prune.sh <package>...   (the packages the guest runs)
#
# The guest executes kata-agent and the handful of tools named on the command line,
# nothing else, and it never runs a package manager. So the keep set is those
# packages, the few below that every image needs, and their Depends/Pre-Depends
# closure. apt works the closure out, not this script: a simulated install of the keep
# set into an empty system lists exactly what it pulls in. Everything installed
# outside it is purged, Essential packages included (perl, util-linux, apt itself...).
#
# dpkg removes the files, so no path in the image is named here, and its database
# still lists exactly what is left, which keeps the image queryable with dpkg and
# scannable by the tools that read /var/lib/dpkg/status. The purged packages'
# prerm/postrm scripts are dropped first: they tidy up debconf, alternatives and
# users on a system that will never run dpkg again, and they call the tools
# (perl, sed, find) this same purge removes, so there is no order that runs them all.

set -o errexit -o nounset -o pipefail

if [[ $# -eq 0 ]]; then
  echo "usage: prune.sh <package>..." >&2
  exit 1
fi

# Always kept:
#   base-files     /etc/os-release and the base directory layout.
#   dash           /bin/sh.
#   dpkg           the package database and dpkg-query, so the guest can say what is in it.
#   libc6, libgcc-s1  kata-agent's own libraries besides libseccomp2 (built with LIBC=gnu).
KEEP=(base-files dash dpkg libc6 libgcc-s1 "$@")

mapfile -t installed < <(dpkg-query -W -f '${Package}\n' | sort -u)
mapfile -t closure < <(
  apt-get --simulate -o Dir::State::status=/dev/null \
    install --no-install-recommends "${KEEP[@]}" |
    awk '$1 == "Inst" {print $2}' | sort -u
)
if [[ ${#closure[@]} -eq 0 ]]; then
  echo "prune.sh: apt resolved an empty closure for: ${KEEP[*]}" >&2
  exit 1
fi

# The simulation starts from an empty system, so where a dependency has alternatives it
# may choose one this image does not have. Keeping that choice would leave the other
# alternative purged, breaking the dependency, so refuse instead: add the installed
# alternative to KEEP.
mapfile -t missing < <(comm -13 <(printf '%s\n' "${installed[@]}") <(printf '%s\n' "${closure[@]}"))
if [[ ${#missing[@]} -gt 0 ]]; then
  echo "prune.sh: the closure of the keep set needs packages this image does not have:" >&2
  echo "  ${missing[*]}" >&2
  echo "Add the installed alternative of the dependency that pulls them in to KEEP." >&2
  exit 1
fi

mapfile -t purge < <(comm -23 <(printf '%s\n' "${installed[@]}") <(printf '%s\n' "${closure[@]}"))
echo ">> Keeping ${#closure[@]} packages: ${closure[*]}"
echo ">> Purging ${#purge[@]} packages: ${purge[*]}"
if [[ ${#purge[@]} -eq 0 ]]; then
  exit 0
fi

for p in "${purge[@]}"; do
  rm -f "/var/lib/dpkg/info/${p}".{prerm,postrm} "/var/lib/dpkg/info/${p}":*.{prerm,postrm}
done
# --no-triggers: the triggers these removals would fire (ldconfig, mostly) belong to
# packages purged alongside them.
dpkg --purge --no-triggers --force-depends --force-remove-essential --force-remove-protected \
  "${purge[@]}"

# dpkg --audit prints nothing when every remaining package is fully installed.
audit="$(dpkg --audit)"
if [[ -n "${audit}" ]]; then
  echo "prune.sh: dpkg --audit after the purge:" >&2
  echo "${audit}" >&2
  exit 1
fi
