name: Whitespace check
on:
  pull_request:
    types:
      - opened
      - reopened
      - synchronize

env:
  error_msg: |+
    See the document below for help on formatting commits for the project.

    https://github.com/kata-containers/community/blob/master/CONTRIBUTING.md#patch-format

jobs:
  whitespace-check:
    runs-on: ubuntu-latest
    name: Whitespace check
    steps:
    - uses: actions/checkout@v2
    - uses: harupy/find-trailing-whitespace@mv1.0
