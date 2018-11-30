# Contributing to Virtual Machine Manager for Go

Virtual Machine Manager for Go is an open source project licensed under the [Apache v2 License] (https://opensource.org/licenses/Apache-2.0)

## Coding Style

Virtual Machine Manager for Go follows the standard formatting recommendations and language idioms set out
in [Effective Go](https://golang.org/doc/effective_go.html) and in the
[Go Code Review Comments wiki](https://github.com/golang/go/wiki/CodeReviewComments).

## Certificate of Origin

In order to get a clear contribution chain of trust we use the [signed-off-by language] (https://01.org/community/signed-process)
used by the Linux kernel project.

## Patch format

Beside the signed-off-by footer, we expect each patch to comply with the following format:

```
Change summary

More detailed explanation of your changes: Why and how.
Wrap it to 72 characters.
See [here] (http://chris.beams.io/posts/git-commit/)
for some more good advices.

Fixes #NUMBER (or URL to the issue)

Signed-off-by: <contributor@foo.com>
```

For example:

```
Fix poorly named identifiers
  
One identifier, fnname, in func.go was poorly named.  It has been renamed
to fnName.  Another identifier retval was not needed and has been removed
entirely.

Fixes #1
    
Signed-off-by: Mark Ryan <mark.d.ryan@intel.com>
```

## New files

Each Go source file in the Virtual Machine Manager for Go project must
contain the following header:

```
/*
// Copyright contributors to the Virtual Machine Manager for Go project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
*/
```

## Contributors File

This CONTRIBUTORS.md file is a partial list of contributors to the
Virtual Machine Manager for Go project. To see the full list of
contributors, see the revision history in source control.

Contributors who wish to be recognized in this file should add
themselves (or their employer, as appropriate).

## Pull requests

We accept github pull requests.

## Quality Controls

We request you give quality assurance some consideration by:

* Adding go unit tests for changes where it makes sense.
* Enabling [Travis CI](https://travis-ci.org/intel/govmm) on your github fork of Virtual Machine Manager for Go to get continuous integration feedback on your dev/test branches.

## Issue tracking

If you have a problem, please let us know.  If it's a bug not already documented, by all means please [open an
issue in github](https://github.com/intel/govmm/issues/new) so we all get visibility
the problem and work toward resolution.

Any security issues discovered with govmm should be reported by following the instructions on https://01.org/security.
