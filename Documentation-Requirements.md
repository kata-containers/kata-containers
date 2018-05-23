* [Introduction](#introduction)
* [General requirements](#general-requirements)
* [Code blocks](#code-blocks)

# Introduction

This document outlines the requirements for all documentation in the [Kata
Containers](https://github.com/kata-containers) project.

# General requirements

- All documents are expected to be written in [GitHub Flavored Markdown](https://github.github.com/gfm) format.
- All documents should have the `.md` file extension.

# Code blocks

This section lists requirements for displaying commands and command output.

The requirements must be adhered to since documentation containing code blocks
is validated by the CI system, which executes the command blocks with the help
of the
[doc-to-script](https://github.com/kata-containers/tests/tree/master/.ci/kata-doc-to-script.sh)
utility.

- If a document includes commands the user should run, they **MUST** be shown
  in a *bash code block* with every command line prefixed with `$ ` to denote
  a prompt:

  ```

      ```bash
      $ echo "Hi - I am some bash code"
      $ sudo docker run -ti busybox true
      $ [ $? -eq 0 ] && echo "success!"
      ```

  ```

- If a command needs to be run as the `root` user, it must be run using
  `sudo(8)`.
  ```bash

  $ sudo echo "I'm running as root"
  ```

- All lines beginning `# ` should be comment lines, *NOT* commands to run as
  the `root` user.

- In the unusual case that you need to display command *output*, use an
  unadorned code block (\`\`\`):

  ```

      The output of the `ls(1)` command is expected to be:

      ```
      ls: cannot access '/foo': No such file or directory
      ```

  ```
