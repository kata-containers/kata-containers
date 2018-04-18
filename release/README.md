# Release tools #

This directory contains tools for Kata Containers releases.

## tag_repos.sh ##

The `tag_repos.sh` script is used to create tags for the Kata Containers
repositories. This script ensures that all the repositories are in the
same version (by checking the `VERSION` file).

The script creates an **annotated tag** for the new release version for
the following repositories:

- agent
- proxy
- runtime
- shim
- throttler

The script also tags the tests and osbuilder repositories to make it clear
which versions of these supporting repositories are used for the release.
