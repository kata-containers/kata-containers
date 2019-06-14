# `checkcommits`

* [Overview](#overview)
* [Handling long lines](#handling-long-lines)
* [Options](#options)
    * [Verbose mode](#verbose-mode)
    * [For full details](#for-full-details)
* [How to use it](#how-to-use-it)
    * [Download and Build](#download-and-build)
    * [Basic use](#basic-use)
    * [Example showing most of the available options:](#example-showing-most-of-the-available-options)
    * [Run under TravisCI](#run-under-travisci)
    * [Run under SemaphoreCI](#run-under-semaphoreci)

## Overview

The `checkcommits` tool is used to perform basic checks on `git(1)`
commits in a repository. It is designed to be run on pull request
branches.

By default, it will ensure:

- A commit subject line is specified.

- The commit subject contains a "subsystem" (adjust using `--subject-length=`).
  A "subsystem" is simply one or more words followed by a colon at the
  start of the subject and designed to identify the area of the code the
  change applies to.

- The commit subject is not overly long (adjust using `--body-length=`).

- A commit body is specified.

- No line in the commit body is overly long.

Optionally, various other checks can be enabled (such as checking for
at least one "Fixes #XXX" entry in the commits and ensuring each commit
contains a developer sign-off).

## Handling long lines

Sometimes, long lines are required in a commit body. For example, a
crash dump or log file extract may easily result in a block of text with
very long lines.

To cater for this scenario, this tool employs a very simply heuristic:
line length limits only apply if the line begins with an alphabetic
character. This has the following attractive properties:

- Log files tend to start with numerics so sometimes get handled
  automatically.

- Emails can be quoted using the usual leading `>` character.

- Any type of information can be made "special" simply by indenting it
  to ensure the first character in the line is not alphanumeric.
  Clearly, this gives committers a way to bypass the checks but the hope
  is they won't stoop to this ;)

Finally, the signed-off lines are never length checked (as it is
unreasonable to penalise people with long names).

## Options

### Verbose mode

By default, no output will be generated unless an error is found.
However, enabling verbose mode will show the commits as they are
checked. To enable verbose mode either specify `--verbose` or set
`CHECKCOMMITS_VERBOSE=1`.

### For full details

Run:

```
$ ./checkcommits -h
```

## How to use it

### Download and Build

```
$ repo="github.com/kata-containers/tests/cmd/checkcommits"
$ go get -d "$repo"
$ (cd "$GOPATH/src/$repo" && make)
```

This will install `checkcommits` into your
[workspace](https://golang.org/doc/code.html#GOPATH)’s `bin` directory,
so ensure `$GOPATH/bin` is in your `$PATH`.

### Basic use

```
$ checkcommits "$commit" "$branch"
```

### Example showing most of the available options:

```
$ checkcommits --verbose --need-fixes --need-sign-offs --body-length 99 --subject-length 42 "$commit" "$branch"
```

### Run under TravisCI

```
$ checkcommits "$TRAVIS_COMMIT" "$TRAVIS_BRANCH"
```

### Run under SemaphoreCI

```
$ checkcommits "$REVISION" "$BRANCH_NAME"
```
