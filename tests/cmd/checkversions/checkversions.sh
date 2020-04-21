#!/usr/bin/env bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Check if our local component versions are out of date with their
# upstream repositories.
#
# Use the Debian uscan (https://manpages.debian.org/stretch/devscripts/uscan.1.en.html)
# to do the compare. As such, uscan format annotations are stored in our version.yaml
# file for any components that should be checked.
# The script recognises the following keys from the versions.yaml file:
#  - 'uscan-url' - the url regexp used to scan for versions of the component
#  - 'uscan-opts' - any additional opts (in the uscan watchfile) required to complete
#    version processing, such as 'filenamemangle'.
#
# This script:
#  - locates any uscan-url keys in the versions.toml file
#  - locates any corresponding 'version' keys (at the same level/entry)
#  - extracts any related 'uscan-opts' if present
#  - constructs a uscan watchfile with the information
#  - executes 'uscan' to gather results
#  - parses the uscan results to determine if an update is available
#  - tabulates the results and prints a report
#  - returns the number of components that could be updated (fails) as the return value

runtime_repo="github.com/kata-containers/runtime"
runtime_repo_dir="$GOPATH/src/${runtime_repo}"
versions_file="${runtime_repo_dir}/versions.yaml"
YQ=$(which yq)
uscan_phrase="uscan-url"

# Store up the paths to all the checks that passed
declare -a check_passed
check_passed=()
# Store up the paths to all the checks that failed
declare -a check_failed
check_failed=()
# Associative array between path names (keys) and the current version
declare -A current_version
current_version=()
declare -A newest_version
newest_version=()

# Record number of passes (up to date) and fails (could be updated)
declare passes
declare fails

# Look in the yaml file handed to us to find uscan URLs.
# return a list of paths to the URL items.
#  $1 - path to versions.yaml file.
#
extract_uscan_items() {
	# We convert the yaml to json, as jq can then be used to extract a list of
	# keys that container our identifying search phrase.
	paths=$(${YQ} r -j "$1" | jq -r 'paths | join(".")' | grep "${uscan_phrase}$")

	echo "$paths"
}

main() {
	passes=0
	fails=0

	local paths=$(extract_uscan_items "$versions_file")

	for path in $paths; do
		# Find the root key path for this component
		local rootname="${path%.$uscan_phrase}"
		echo "Processing [$rootname]"
		local versionpath="$rootname.version"
		longversion=$(yq r $versions_file $versionpath)
		# Strip any leading non-digit values - uscan deals with digit style versions
		# and copes better without any prefixes
		version=$(sed -E 's/^[^0-9]+//' <<< $longversion)
		# store away to use in the report table
		current_version[$path]="$version"
		uscanurl=$(yq r $versions_file $path)
		local optspath="$rootname.uscan-opts"
		uscanopts=$(yq r $versions_file $optspath)

		# uscan needs a 'version' entry at the top of its file. uscan currently always
		# expects that to be version 4.
		echo "version=4" > watchfile
		if [ -n "$uscanopts" ] && [ "$uscanopts" != "null" ]; then
			echo "$uscanopts  \\" >> watchfile
		fi
		echo "$uscanurl" >> watchfile

		# And run the uscan
		result="$(uscan --report --package ${rootname} --upstream-version ${version} --watchfile watchfile 2>/dev/null)"
		
		# Extract the latest version found
		newversion="$(egrep -o 'remote site is (.*)+,' <<< $result | sed 's/remote site is //' | sed 's/,//')"
		newest_version[$path]="$newversion"

		# And note if that was marked as being 'Newer' than our current version
		failure=$(grep Newer <<< $result)

		# Store the results for later
		if [ -n "$failure" ]; then
			((fails++))
			check_failed[${#check_failed[@]}]="$path"
		else
			((passes++))
			check_passed[${#check_passed[@]}]="$path"
		fi
	done
}

# Generate a nice tabulated summary of the results.
results() {
	printf "\n %3s: %40s %20s %20s\n" "Num" "Item" "Current Ver"  "Upstream Ver"
	if [ $passes -gt 0 ]; then
		echo "PASSES:"
		for (( index=0; index<$passes; index++ )); do
			path="${check_passed[$index]}"
			printf " %3d: %40s %20s %20s\n" "$index" "${check_passed[$index]}" "${current_version[$path]}"  "${newest_version[$path]}"
		done
	fi

	echo ""
	if [ $fails -gt 0 ]; then
		echo "FAILURES:"
		for (( index=0; index<$fails; index++ )); do
			path="${check_failed[$index]}"
			printf " %3d: %40s %20s %20s\n" "$index" "${check_failed[$index]}" "${current_version[$path]}"  "${newest_version[$path]}"
		done
	fi

	echo ""
	echo "PASSES: $passes"
	echo "FAILS: $fails"
}

main
results
exit $fails
