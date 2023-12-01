# GitHub labels archive

## Overview

This directory contains one YAML file per repository containing the original
set of GitHub labels before the
[new ones were applied on 2019-06-04](../labels.yaml.in).

## How the YAML files were created

This section explains how the YAML files were created.

The [`labeler`](https://github.com/tonglil/labeler) tool was used to read
the labels and write them to a YAML file.

### Install and patch the `labeler` tool

This isn't ideal but our [labels database](../labels.yaml.in) mandates
descriptions for every label. However, at the time of writing, the `labeler`
tool does not support descriptions. But,
[there is a PR](https://github.com/tonglil/labeler/pull/37)
to add in description support.

To enable description support:

```sh
$ go get -u github.com/tonglil/labeler
$ cd $GOPATH/src/github.com/tonglil/labeler
$ pr=37
$ pr_branch="PR${pr}"
$ git fetch origin "refs/pull/${pr}/head:{pr_branch}"
$ git checkout "${pr_branch}"
$ go install -v ./...
```

### Save GitHub labels for a repository

Run the following for reach repository:

```sh
$ labeler scan -r ${github_repo_slug} ${output_file}
```

For example, to save the labels for the `tests` repository:

```sh
$ labeler scan -r kata-containers/tests tests.yaml

```

