# history.sh

A simple script to help extract git log history from the main Kata Containers
GitHub repos and sort them by date.

This can be very helpful for instance when:

- Searching or bisecting recent commits that may have caused a regression.
- Examining recent commits that may be suitable for stable backports

The script uses `git fetch` to try and have minimal impact on any existing
repositories and state.

The script uses the `$GOPATH/src/github.com/kata-containers/` repo paths.

## Arguments

The script can take a number of command line arguments or environment variables
to configure a number of parameters, including:

- The date range to extract commit information from.
- The git remote and branch to extract commit information from.

See the script source or `history.sh -h` for more information.
