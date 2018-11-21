* [Introduction](#introduction)
* [General requirements](#general-requirements)
* [Notes](#notes)
* [Warnings and other admonitions](#warnings-and-other-admonitions)
* [Files and command names](#files-and-command-names)
* [Code blocks](#code-blocks)
* [Images](#images)

# Introduction

This document outlines the requirements for all documentation in the [Kata
Containers](https://github.com/kata-containers) project.

# General requirements

- All documents are expected to be written in [GitHub Flavored Markdown](https://github.github.com/gfm) format.
- All documents should have the `.md` file extension.

# Notes

Important information that is not part of the main document flow should be
added as a Note in bold with all content contained within a block quote:

> **Note:** This is areally important point!
>
> This particular note also spans multiple lines. The entire note should be
> included inside the quoted block.

If there are multiple notes, bullets should be used:

> **Notes:**
>
> - I am important point 1.
>
> - I am important point 2.
>
> - I am important point *n*.

# Warnings and other admonitions

Use the same approach as for [notes](#notes). For example:

> **Warning:** Running this command assumes you understand the risks of doing so.

Other examples:

> **Warnings:**
>
> - Do not unplug your computer!
> - Always read the label.
> - Do not pass go. Do not collect $200.

> **Tip:** Read the manual page for further information on available options.

> **Hint:** Look behind you!

# Files and command names

All filenames and command names should be rendered in a fixed-format font
using backticks:

> Run the `foo` command to make it work.

> Modify the `bar` option in file `/etc/baz/baz.conf`.

Render any options that need to be specified to the command in the same manner:

> Run `bar -axz --apply foo.yaml` to make the changes.

For standard system commands, it is also acceptable to specify the name along
with the manual page section that documents the command in brackets:

> The command to list files in a directory is called `ls(1)`.

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
      $ [ $? -eq 0 ] && echo "success"
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

- Long lines should not span across multiple lines by using the '`\`'
  continuation character.

  GitHub automatically renders such blocks with scrollbars. Consequently,
  backslash continuation characters are not necessary and are a visual
  distraction. These characters also mess up a user's shell history when
  commands are pasted into a terminal.

# Images

All binary image files must be in a standard and well-supported format such as
PNG. This format is preferred for vector graphics such as diagrams because the
information is stored more efficiently, leading to smaller file sizes. JPEG
images are acceptable, but this format is more appropriate to store
photographic images.

When possible, generate images using freely available software.

Every binary image file **MUST** be accompanied by the "source" file used to
generate it. This guarantees that the image can be modified by updating the
source file and re-generating the binary format image file.

Ideally, the format of all image source files is an open standard, non-binary
one such as SVG. Text formats are highly preferable because you can manipulate
and compare them with standard tools (e.g. `diff(1)`).
