# checkmetrics/history.sh

A simple script that extracts historical metrics test data from the
Jenkins CI server and generates a set of statistical data that can then
be used to adjust the metrics CI baseline data.

## Arguments

The script can take a number of command line arguments to set the
Jenkins jobs to examine, and how far back in history to look.

If the metrics data evaluated needs to be changed, this has to be
edited in the script file directly, as it requires updating matching
pairs of test names and `jq` JSON query strings.

See the script source or `history.sh -h` for more information.
