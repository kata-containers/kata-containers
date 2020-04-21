#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Extract git history from the main Kata repos and generate two report files:
#  report.txt - commits merged, sorted by repo section, and date.
#  sorted_report.txt - all commits sorted by date
#
# This information can be useful when tracking down 'what changed when'.

set -e

REPORTFILE="report.txt"
SORTEDREPORTFILE="sorted_${REPORTFILE}"

# Set a date to start from, or use the default of '1 week'
# Format is in any acceptible by 'Date -I -d'.
defstart="${defstart:-last week}"
defend="${defend:-now}"
START=${START:-$(date -I -d "${defstart}")}
END=${END:-$(date -I -d "$defend")}

# Where are we looking? Default to origin/master
remote="${remote:-origin}"
branch="${branch:-master}"

repo_base="github.com/kata-containers"
repos="${repos:-
	agent \
	osbuilder \
	packaging \
	proxy \
	runtime \
	shim \
	tests \
	}"

msg() {
	local msg="$*"
	echo "${msg}" | tee -a "${REPORTFILE}"
}

run() {
	# Blank the file
	echo "" > "${REPORTFILE}"
	
	msg "------------------------------------"
	msg "Commits from $START to $END"
	msg "------------------------------------"
	for repo in $repos; do
		repopath="${GOPATH}/src/${repo_base}/${repo}"
		git -C "${repopath}" fetch ${remote} ${branch}
		msg "---------- $repo ---------------"
		TZ=UTC git -C "${repopath}" log --since "$START" --until "$END" --pretty="%cd: %h: ${repo}: %s" --date=format:"%Y-%m-%dT%H:%M:%S" --no-merges ${branch} | tee -a "${REPORTFILE}"
		msg ""
	done
	
	# And make a date sorted version of the file
	sort -r < "${REPORTFILE}" > "${SORTEDREPORTFILE}"
	# Remove all the 'fluff' from that sorted file, as it is now just noise.
	sed -i '/^---.*$/d' "${SORTEDREPORTFILE}"
	sed -i '/^$/d' "${SORTEDREPORTFILE}"
}


help() {
	usage=$(cat << EOF
Usage: $0 [-h] [options]
   Description:
        Extract key Kata Containers github repository history and
        format it into a human readable format.
        The script uses `git fetch` to gather up-to date information,
        thus avoiding modifications to existing repos.

        Date argument formats are anything acceptible to 'git log'.
        Output generated into files [${REPORTFILE}] and [${SORTEDREPORTFILE}].

   Options:
        -b <branch>, Which branch to extract history from (default: ${branch})
        -f <date>,   From which date (default: $defstart)
        -h,          Print this help
        -r <remote>, Which git remote to fetch from (default: ${remote})
        -t <date>,   To which date (default: $defend)
EOF
)
	echo "$usage"
}


main() {
	local OPTIND
	while getopts "b:f:hr:t:" opt;do
		case ${opt} in
		b)
		    branch=${OPTARG}
		    ;;
		f)
		    START="${OPTARG}"
		    ;;
		h)
		    help
		    exit 0;
		    ;;
		r)
		    remote="${OPTARG}"
		    ;;
		t)
		    END="${OPTARG}"
		    ;;
		?)
		    # parse failure
		    help
		    echo "Failed to parse arguments" >&2
		    exit -1
		    ;;
		esac
	done
	shift $((OPTIND-1))

	run
}

main "$@"
