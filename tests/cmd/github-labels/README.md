# Overview

The Kata Project uses a number of GitHub repositories. To allow issues and PRs
to be handled consistently between repositories a standard set of issue labels
are used. These labels are stored in YAML format in the master
[labels database template](labels.yaml.in). This file is human-readable,
machine-readable, and self-describing (see the file for the introductory
description).

Each repository can contain a set of additional (repository-specific) labels,
which are stored in a top-level YAML template file called `labels.yaml.in`.

Expanding the templates and merging the two databases describes the full set
of labels a repository uses.

# Generating the combined labels database

You can run the `github_labels.sh` script with the `generate` argument to
create the combined labels database. The additional arguments specify the
repository (in order to generate the combined labels database) and the name of
a file to write the combined database:

```sh
$ ./github-labels.sh generate github.com/kata-containers/kata-containers /tmp/combined.yaml
```

This script validates the combined labels database by performing a number of
checks, including running the `kata-github-labels` tool in checking mode. See
the
[Checking and summarising the labels database](#checking-and-summarising-the-labels-database)
section for more information.

# Checking and summarising the labels database

The `kata-github-labels` tool checks and summarizes the labels database for
each repository.

## Show labels

Displays a summary of the labels:

```sh
$ kata-github-labels show labels labels.yaml
```

## Show categories

Shows all information about categories:

```sh
$ kata-github-labels show categories --with-labels labels.yaml
```
## Check only

Performs checks on a specified labels database:

```sh
$ kata-github-labels check labels.yaml
```

## Full details

Lists all available options:

```sh
$ kata-github-labels -h
```

# Archive of old GitHub labels

See the [archive documentation](archive).
