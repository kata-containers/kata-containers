# Introduction

This document outlines the requirements for all documentation in the [Kata
Containers](https://github.com/kata-containers) project.

# General requirements

All documents must:

- Be written in simple English.
- Be written in [GitHub Flavored Markdown](https://github.github.com/gfm) format.
- Have a `.md` file extension.
- Be linked to from another document in the same repository.

  Although GitHub allows navigation of the entire repository, it should be
  possible to access all documentation purely by navigating links inside the
  documents, starting from the repositories top-level `README`.

  If you are adding a new document, ensure you add a link to it in the
  "closest" `README` above the directory where you created your document.
- If the document needs to tell the user to manipulate files or commands, use a
  [code block](#code-blocks) to specify the commands.

  If at all possible, ensure that every command in the code blocks can be run
  non-interactively. If this is possible, the document can be tested by the CI
  which can then execute the commands specified to ensure the instructions are
  correct. This avoids documents becoming out of date over time.

> **Note:**
>
> Do not add a table of contents (TOC) since GitHub will auto-generate one.

# Linking advice

Linking between documents is strongly encouraged to help users and developers
navigate the material more easily. Linking also avoids repetition - if a
document needs to refer to a concept already well described in another section
or document, do not repeat it, link to it
(the [DRY](https://en.wikipedia.org/wiki/Don%27t_repeat_yourself) principle).

Another advantage of this approach is that changes only need to be applied in
one place: where the concept is defined (not the potentially many places where
the concept is referred to using a link).

# Notes

Important information that is not part of the main document flow should be
added as a Note in bold with all content contained within a block quote:

> **Note:** This is a really important point!
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
[doc-to-script](https://github.com/kata-containers/kata-containers/blob/main/tests/kata-doc-to-script.sh)
utility.

- If a document includes commands the user should run, they **MUST** be shown
  in a *bash code block* with every command line prefixed with `$ ` to denote
  a shell prompt:

  <pre>

      ```bash
      $ echo "Hi - I am some bash code"
      $ sudo docker run -ti busybox true
      $ [ $? -eq 0 ] && echo "success"
      ```

  <pre>

- If a command needs to be run as the `root` user, it must be run using
  `sudo(8)`.

  ```bash

  $ sudo echo "I'm running as root"
  ```

- All lines beginning `# ` should be comment lines, *NOT* commands to run as
  the `root` user.

- Try to avoid showing the *output* of commands.

  The reasons for this:

  - Command output can change, leading to confusion when the output the user
    sees does not match the output in the documentation.

  - There is the risk the user will get confused between what parts of the
    block refer to the commands they should type and the output that they
    should not.

  - It can make the document look overly "busy" or complex.

  In the unusual case that you need to display command *output*, use an
  unadorned code block (\`\`\`):

  <pre>

      The output of the `ls(1)` command is expected to be:

      ```
      ls: cannot access '/foo': No such file or directory
      ```

  <pre>

- Long lines should not span across multiple lines by using the `\`
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

# Spelling

Since this project uses a number of terms not found in conventional
dictionaries, we have a
[spell checking tool](https://github.com/kata-containers/kata-containers/tree/main/tests/cmd/check-spelling)
that checks both dictionary words and the additional terms we use.

Run the spell checking tool on your document before raising a PR to ensure it
is free of mistakes.

If your document introduces new terms, you need to update the custom
dictionary used by the spell checking tool to incorporate the new words.

# Names

Occasionally documents need to specify the name of people. Write such names in
backticks. The main reason for this is to keep the [spell checker](#spelling) happy (since
it cannot manage all possible names). However, since backticks render in a
fixed-width font, this makes the names clearer:

> Welcome to `Clark Kent`, the newest member of the Kata Containers Architecture Committee.

# Version numbers

Write version number in backticks. This keeps the [spell checker](#spelling)
happy and since backticks render in a fixed-width font, it also makes the
numbers clearer:

> Ensure you are using at least version `1.2.3-alpha3.wibble.1` of the tool.

# The apostrophe

The apostrophe character (`'`) must **only** be used for showing possession
("Peter's book") and for standard contractions (such as "don't").

Use double-quotes ("...") in all other circumstances you use quotes outside of
[code blocks](#code-blocks).
