# `kata-manager`

* [Overview](#overview)
    * [For full details](#for-full-details)
* [Warning](#warning)
* [How to use it](#how-to-use-it)
    * [Download](#download)
    * [Basic use](#basic-use)

## Overview

The [`kata-manager`](kata-manager.sh) is a tool that can be used to perform
common tasks such as installing the packaged version of [Kata
Containers](https://github.com/kata-containers), installing a container
manager and configuring the runtime.

Unlike normal scripts which specify the steps to run, for installation tasks
`kata-manager` calling the [`kata-doc-to-script`](/.ci/kata-doc-to-script.sh)
tool which parses well-formed [GitHub Flavored
Markdown](https://github.github.com/gfm) format documents that contain `bash`
code blocks and converts them into a script. This allows `kata-manager` for
example to convert a Kata installation guide into a script and execute it. Not
only is this useful for users, since the `kata-manager` is run from the CI
system, the installation guides are assured of being correct.

### For full details

Run:

```
$ ./kata-manager -h
```

## Warning

> **Note:** Since in some cases `kata-manager` is consuming documents and
> converting their contents, there is some risk associated with running the
> script. We recommend that if you choose to use `kata-manager` that you only
> do so on non-critical system. Further, you are encouraged to run
> `kata-manager` using the `-n` option which will generate the scripts, but
> will not execute them. This allows you to review the scripts before running
> them.

## How to use it

### Download

```
$ repo="github.com/kata-containers/tests"
$ go get -d "$repo"
$ PATH=$PATH:$GOPATH/src/${repo}/cmd/kata-manager
```

This will add the `kata-manager.sh` to your `$PATH`.

### Basic use

```
$ kata-manager.sh install-packages
```
